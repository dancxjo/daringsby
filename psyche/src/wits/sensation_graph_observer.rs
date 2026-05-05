#[cfg(feature = "face")]
use crate::sensors::face::FaceInfo;
use crate::traits::observer::SensationObserver;
use crate::wits::memory::GraphStore;
use crate::{
    AudioClip, GeoEmbedding, GeoLoc, ImageData, ImageEmbedding, Sensation, Topic, TopicBus,
    VoiceInfo, audio_clip_id, geoloc_content_id, image_content_id,
};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::warn;

/// Stores raw sensory artifacts as deterministic graph merge records.
pub struct SensationGraphObserver {
    graph: Arc<dyn GraphStore>,
    seen: Mutex<HashSet<String>>,
}

impl SensationGraphObserver {
    pub fn new(graph: Arc<dyn GraphStore>) -> Self {
        Self {
            graph,
            seen: Mutex::new(HashSet::new()),
        }
    }

    /// Also observe sensations published directly onto the topic bus by vector pipelines.
    pub fn spawn_topic_listener(self: Arc<Self>, bus: TopicBus) {
        tokio::spawn(async move {
            let stream = bus.subscribe(Topic::Sensation);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Some(sensation) = payload.downcast_ref::<Sensation>() {
                    self.observe_sensation(sensation).await;
                } else if let Some(sensation) = payload.downcast_ref::<Arc<Sensation>>() {
                    self.observe_sensation(sensation.as_ref()).await;
                }
            }
        });
    }

    async fn store_once(&self, key: String, record: Value) {
        {
            let mut seen = self.seen.lock().unwrap();
            if !seen.insert(key) {
                return;
            }
        }
        if let Err(e) = self.graph.store_data(&record).await {
            warn!(?e, "graph sensation store failed");
        }
    }
}

#[async_trait]
impl SensationObserver for SensationGraphObserver {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        let Some(Sensation::Of {
            payload,
            occurred_at,
        }) = payload.downcast_ref::<Sensation>()
        else {
            return;
        };

        if let Some(image) = payload.downcast_ref::<ImageData>() {
            let id = image_content_id(image);
            self.store_once(
                format!("image:{id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [image_node(image, &id, occurred_at.to_rfc3339())],
                    "relationships": [],
                }),
            )
            .await;
        } else if let Some(loc) = payload.downcast_ref::<GeoLoc>() {
            let id = geoloc_content_id(loc);
            self.store_once(
                format!("geolocation:{id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [geolocation_node(loc, &id, occurred_at.to_rfc3339())],
                    "relationships": [],
                }),
            )
            .await;
        } else if let Some(geo_embedding) = payload.downcast_ref::<GeoEmbedding>() {
            let vector_id = geo_embedding
                .vector_id
                .clone()
                .unwrap_or_else(|| format!("geolocation-vector:{}", geo_embedding.geoloc_id));
            self.store_once(
                format!("geolocation_embedding:{vector_id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        geolocation_node(
                            &geo_embedding.loc,
                            &geo_embedding.geoloc_id,
                            occurred_at.to_rfc3339(),
                        ),
                        {
                            "label": "Vector",
                            "id": vector_id,
                            "collection": "geolocations",
                            "kind": "geolocation",
                            "model": geo_embedding.model.clone(),
                        }
                    ],
                    "relationships": [{
                        "from": geo_embedding.geoloc_id,
                        "to": vector_id,
                        "type": "HAS_GEOLOCATION_VECTOR",
                    }],
                }),
            )
            .await;
        } else if let Some(image_embedding) = payload.downcast_ref::<ImageEmbedding>() {
            let vector_id = image_embedding
                .vector_id
                .clone()
                .unwrap_or_else(|| format!("image-vector:{}", image_embedding.image_id));
            self.store_once(
                format!("image_embedding:{vector_id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        image_node(
                            &image_embedding.image,
                            &image_embedding.image_id,
                            occurred_at.to_rfc3339(),
                        ),
                        {
                            "label": "Vector",
                            "id": vector_id,
                            "collection": "images",
                            "kind": "image",
                            "model": image_embedding.model.clone(),
                        }
                    ],
                    "relationships": [{
                        "from": image_embedding.image_id,
                        "to": vector_id,
                        "type": "HAS_IMAGE_VECTOR",
                    }],
                }),
            )
            .await;
        } else {
            #[cfg(feature = "face")]
            if let Some(face) = payload.downcast_ref::<FaceInfo>() {
                let vector_id = face
                    .vector_id
                    .clone()
                    .unwrap_or_else(|| format!("face-vector:{}", face.face_id));
                self.store_once(
                    format!("face:{}", face.face_id),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            image_node(&face.crop, &face.face_id, occurred_at.to_rfc3339()),
                            {
                                "label": "Vector",
                                "id": vector_id,
                                "collection": "faces",
                                "kind": "face",
                            }
                        ],
                        "relationships": [
                            {
                                "from": face.source_image_id,
                                "to": face.face_id,
                                "type": "CONTAINS_FACE",
                            },
                            {
                                "from": face.face_id,
                                "to": vector_id,
                                "type": "HAS_FACE_VECTOR",
                            }
                        ],
                    }),
                )
                .await;
                return;
            }
            if let Some(voice) = payload.downcast_ref::<VoiceInfo>() {
                store_voice(self, voice, occurred_at.to_rfc3339()).await;
            } else if let Some(audio) = payload.downcast_ref::<AudioClip>() {
                let clip_id = audio_clip_id(audio);
                self.store_once(
                    format!("audio:{clip_id}"),
                    json!({
                        "op": "merge_graph",
                        "nodes": [audio_node(audio, &clip_id, occurred_at.to_rfc3339())],
                        "relationships": [],
                    }),
                )
                .await;
            }
        }
    }
}

async fn store_voice(observer: &SensationGraphObserver, voice: &VoiceInfo, occurred_at: String) {
    let vector_id = voice
        .vector_id
        .clone()
        .unwrap_or_else(|| format!("voice-vector:{}", voice.clip_id));
    observer
        .store_once(
            format!("voice:{}", voice.clip_id),
            json!({
                "op": "merge_graph",
                "nodes": [
                    audio_node(&voice.clip, &voice.clip_id, occurred_at),
                    {
                        "label": "Vector",
                        "id": vector_id,
                        "collection": "voices",
                        "kind": "voice",
                        "model": voice.model.clone(),
                    }
                ],
                "relationships": [{
                    "from": voice.clip_id,
                    "to": vector_id,
                    "type": "HAS_VOICE_VECTOR",
                }],
            }),
        )
        .await;
}

fn image_node(image: &ImageData, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Image",
        "id": id,
        "merge_key": "id",
        "mime": image.mime.clone(),
        "captured_at": image.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn audio_node(audio: &AudioClip, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "AudioClip",
        "id": id,
        "merge_key": "id",
        "mime": audio.mime.clone(),
        "sample_rate": audio.sample_rate,
        "channels": audio.channels,
        "captured_at": audio.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn geolocation_node(loc: &GeoLoc, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Geolocation",
        "id": id,
        "merge_key": "id",
        "latitude": loc.latitude,
        "longitude": loc.longitude,
        "observed_at": loc.observed_at.clone(),
        "occurred_at": occurred_at,
    })
}
