#[cfg(feature = "face")]
use crate::sensors::face::FaceInfo;
use crate::traits::observer::SensationObserver;
use crate::wits::memory::{GraphStore, qdrant_vector_node};
use crate::{
    AudioClip, GeoEmbedding, GeoLoc, Heartbeat, ImageData, ImageEmbedding, ObjectInfo, Sensation,
    Topic, TopicBus, VoiceInfo, audio_clip_id, geoloc_content_id, image_content_id,
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
        let Some(sensation) = payload.downcast_ref::<Sensation>() else {
            return;
        };

        let Sensation::Of {
            payload,
            occurred_at,
        } = sensation
        else {
            store_spoken_sensation(self, sensation).await;
            return;
        };

        if let Some(image) = payload.downcast_ref::<ImageData>() {
            let id = image_content_id(image);
            let sensation_id = sensation_id("image", &id, occurred_at.to_rfc3339());
            self.store_once(
                format!("image:{id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(&sensation_id, "image", occurred_at.to_rfc3339()),
                        image_node(image, &id, occurred_at.to_rfc3339()),
                    ],
                    "relationships": [{
                        "from": sensation_id,
                        "to": id,
                        "type": "OBSERVED",
                    }],
                }),
            )
            .await;
        } else if let Some(loc) = payload.downcast_ref::<GeoLoc>() {
            let id = geoloc_content_id(loc);
            let sensation_id = sensation_id("geolocation", &id, occurred_at.to_rfc3339());
            self.store_once(
                format!("geolocation:{id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(&sensation_id, "geolocation", occurred_at.to_rfc3339()),
                        geolocation_node(loc, &id, occurred_at.to_rfc3339()),
                    ],
                    "relationships": [{
                        "from": sensation_id,
                        "to": id,
                        "type": "OBSERVED",
                    }],
                }),
            )
            .await;
        } else if let Some(geo_embedding) = payload.downcast_ref::<GeoEmbedding>() {
            let point_id = geo_embedding
                .vector_id
                .clone()
                .unwrap_or_else(|| format!("geolocation-vector:{}", geo_embedding.geoloc_id));
            let vector_id = crate::wits::memory::qdrant_vector_node_id("geolocations", &point_id);
            let sensation_id = sensation_id(
                "geolocation_embedding",
                &vector_id,
                occurred_at.to_rfc3339(),
            );
            self.store_once(
                format!("geolocation_embedding:{vector_id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(
                            &sensation_id,
                            "geolocation_embedding",
                            occurred_at.to_rfc3339(),
                        ),
                        geolocation_node(
                            &geo_embedding.loc,
                            &geo_embedding.geoloc_id,
                            occurred_at.to_rfc3339(),
                        ),
                        qdrant_vector_node(
                            "geolocations",
                            &point_id,
                            "geolocation",
                            geo_embedding.model.as_deref(),
                        )
                    ],
                    "relationships": [
                        {
                            "from": sensation_id,
                            "to": vector_id,
                            "type": "PRODUCED",
                        },
                        {
                            "from": geo_embedding.geoloc_id,
                            "to": vector_id,
                            "type": "HAS_GEOLOCATION_VECTOR",
                        }
                    ],
                }),
            )
            .await;
        } else if let Some(image_embedding) = payload.downcast_ref::<ImageEmbedding>() {
            let point_id = image_embedding
                .vector_id
                .clone()
                .unwrap_or_else(|| format!("image-vector:{}", image_embedding.image_id));
            let vector_id = crate::wits::memory::qdrant_vector_node_id("images", &point_id);
            let sensation_id =
                sensation_id("image_embedding", &vector_id, occurred_at.to_rfc3339());
            self.store_once(
                format!("image_embedding:{vector_id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(&sensation_id, "image_embedding", occurred_at.to_rfc3339()),
                        image_node(
                            &image_embedding.image,
                            &image_embedding.image_id,
                            occurred_at.to_rfc3339(),
                        ),
                        qdrant_vector_node(
                            "images",
                            &point_id,
                            "image",
                            image_embedding.model.as_deref(),
                        )
                    ],
                    "relationships": [
                        {
                            "from": sensation_id,
                            "to": vector_id,
                            "type": "PRODUCED",
                        },
                        {
                            "from": image_embedding.image_id,
                            "to": vector_id,
                            "type": "HAS_IMAGE_VECTOR",
                        }
                    ],
                }),
            )
            .await;
        } else {
            #[cfg(feature = "face")]
            if let Some(face) = payload.downcast_ref::<FaceInfo>() {
                let point_id = face
                    .vector_id
                    .clone()
                    .unwrap_or_else(|| format!("face-vector:{}", face.face_id));
                let vector_id = crate::wits::memory::qdrant_vector_node_id("faces", &point_id);
                let sensation_id = sensation_id("face", &face.face_id, occurred_at.to_rfc3339());
                self.store_once(
                    format!("face:{}", face.face_id),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(&sensation_id, "face", occurred_at.to_rfc3339()),
                            {
                                "label": "Image",
                                "id": face.source_image_id,
                            },
                            face_node(face, occurred_at.to_rfc3339()),
                            qdrant_vector_node("faces", &point_id, "face", None),
                        ],
                        "relationships": [
                            {
                                "from": sensation_id,
                                "to": face.face_id,
                                "type": "OBSERVED",
                            },
                            {
                                "from": face.source_image_id,
                                "to": face.face_id,
                                "type": "CONTAINS_FACE",
                            },
                            {
                                "from": face.face_id,
                                "to": face.source_image_id,
                                "type": "DERIVED_FROM",
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
                let sensation_id = sensation_id("audio", &clip_id, occurred_at.to_rfc3339());
                self.store_once(
                    format!("audio:{clip_id}"),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(&sensation_id, "audio", occurred_at.to_rfc3339()),
                            audio_node(audio, &clip_id, occurred_at.to_rfc3339()),
                        ],
                        "relationships": [{
                            "from": sensation_id,
                            "to": clip_id,
                            "type": "OBSERVED",
                        }],
                    }),
                )
                .await;
            } else if let Some(heartbeat) = payload.downcast_ref::<Heartbeat>() {
                let id = format!("heartbeat:{}", heartbeat.timestamp.to_rfc3339());
                let sensation_id = sensation_id("heartbeat", &id, occurred_at.to_rfc3339());
                self.store_once(
                    id.clone(),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(&sensation_id, "heartbeat", occurred_at.to_rfc3339()),
                            heartbeat_node(heartbeat, &id, occurred_at.to_rfc3339()),
                        ],
                        "relationships": [{
                            "from": sensation_id,
                            "to": id,
                            "type": "OBSERVED",
                        }],
                    }),
                )
                .await;
            } else if let Some(object) = payload.downcast_ref::<ObjectInfo>() {
                let id = object_info_id(object, occurred_at.to_rfc3339());
                let sensation_id = sensation_id("object", &id, occurred_at.to_rfc3339());
                self.store_once(
                    id.clone(),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(&sensation_id, "object", occurred_at.to_rfc3339()),
                            object_info_node(object, &id, occurred_at.to_rfc3339()),
                        ],
                        "relationships": [{
                            "from": sensation_id,
                            "to": id,
                            "type": "OBSERVED",
                        }],
                    }),
                )
                .await;
            } else {
                store_unknown_sensation(self, payload.type_id(), occurred_at.to_rfc3339()).await;
            }
        }
    }
}

async fn store_voice(observer: &SensationGraphObserver, voice: &VoiceInfo, occurred_at: String) {
    let point_id = voice
        .vector_id
        .clone()
        .unwrap_or_else(|| format!("voice-vector:{}", voice.clip_id));
    let vector_id = crate::wits::memory::qdrant_vector_node_id("voices", &point_id);
    let sensation_id = sensation_id("voice", &voice.clip_id, occurred_at.clone());
    observer
        .store_once(
            format!("voice:{}", voice.clip_id),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(&sensation_id, "voice", occurred_at.clone()),
                    audio_node(&voice.clip, &voice.clip_id, occurred_at),
                    qdrant_vector_node(
                        "voices",
                        &point_id,
                        "voice",
                        voice.model.as_deref(),
                    )
                ],
                "relationships": [
                    {
                        "from": sensation_id,
                        "to": vector_id,
                        "type": "PRODUCED",
                    },
                    {
                        "from": voice.clip_id,
                        "to": vector_id,
                        "type": "HAS_VOICE_VECTOR",
                    }
                ],
            }),
        )
        .await;
}

async fn store_spoken_sensation(observer: &SensationGraphObserver, sensation: &Sensation) {
    let (speaker, text, occurred_at) = match sensation {
        Sensation::HeardOwnVoice { text, occurred_at } => ("self", text, occurred_at),
        Sensation::HeardUserVoice { text, occurred_at } => ("user", text, occurred_at),
        Sensation::Of { .. } => return,
    };
    let utterance_id = format!("utterance:{speaker}:{}:{text}", occurred_at.to_rfc3339());
    let sensation_id = sensation_id("utterance", &utterance_id, occurred_at.to_rfc3339());
    observer
        .store_once(
            format!("utterance:{speaker}:{}:{text}", occurred_at.to_rfc3339()),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(&sensation_id, "utterance", occurred_at.to_rfc3339()),
                    {
                        "label": "Utterance",
                        "id": utterance_id,
                        "speaker": speaker,
                        "text": text,
                        "occurred_at": occurred_at.to_rfc3339(),
                    }
                ],
                "relationships": [{
                    "from": sensation_id,
                    "to": utterance_id,
                    "type": "OBSERVED",
                }],
            }),
        )
        .await;
}

fn sensation_node(id: &str, kind: &str, occurred_at: String) -> Value {
    json!({
        "label": "Sensation",
        "id": id,
        "kind": kind,
        "occurred_at": occurred_at,
    })
}

fn heartbeat_node(heartbeat: &Heartbeat, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Heartbeat",
        "id": id,
        "timestamp": heartbeat.timestamp.to_rfc3339(),
        "occurred_at": occurred_at,
    })
}

fn object_info_node(object: &ObjectInfo, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "ObjectObservation",
        "id": id,
        "object_label": object.label.clone(),
        "embedding_len": object.embedding.len(),
        "occurred_at": occurred_at,
    })
}

fn object_info_id(object: &ObjectInfo, occurred_at: String) -> String {
    format!(
        "object:{}:{}:{}",
        object.label.clone().unwrap_or_else(|| "unknown".into()),
        object.embedding.len(),
        occurred_at
    )
}

async fn store_unknown_sensation(
    observer: &SensationGraphObserver,
    type_id: std::any::TypeId,
    occurred_at: String,
) {
    let type_id = format!("{type_id:?}");
    let id = format!("unknown-sensation:{type_id}:{occurred_at}");
    let sensation_id = sensation_id("unknown", &id, occurred_at.clone());
    observer
        .store_once(
            id.clone(),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(&sensation_id, "unknown", occurred_at.clone()),
                    {
                        "label": "UnknownSensation",
                        "id": id,
                        "type_id": type_id,
                        "occurred_at": occurred_at,
                    }
                ],
                "relationships": [{
                    "from": sensation_id,
                    "to": id,
                    "type": "OBSERVED",
                }],
            }),
        )
        .await;
}

#[cfg(feature = "face")]
fn face_node(face: &FaceInfo, occurred_at: String) -> Value {
    json!({
        "label": "Face",
        "id": face.face_id,
        "source_image_id": face.source_image_id,
        "crop_mime": face.crop.mime.clone(),
        "crop_base64": face.crop.base64.clone(),
        "captured_at": face.crop.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn sensation_id(kind: &str, content_id: &str, occurred_at: String) -> String {
    format!("sensation:{kind}:{content_id}:{occurred_at}")
}

fn image_node(image: &ImageData, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "Image",
        "id": id,
        "merge_key": "id",
        "mime": image.mime.clone(),
        "base64": image.base64.clone(),
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
        "base64": audio.base64.clone(),
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
