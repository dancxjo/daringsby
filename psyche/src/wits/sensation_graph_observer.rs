#[cfg(feature = "face")]
use crate::sensors::face::FaceInfo;
use crate::traits::observer::SensationObserver;
use crate::wits::memory::GraphStore;
use crate::{
    AudioClip, BrowserMotion, CombobulationSummary, GeoEmbedding, GeoLoc, Heartbeat, ImageData,
    ImageEmbedding, Impression, ObjectInfo, Sensation, Topic, TopicBus, VoiceInfo, WillContext,
    audio_clip_id, browser_motion_content_id, geoloc_content_id, image_content_id,
};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
            store_textual_sensation(self, sensation).await;
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
                        sensation_node(
                            &sensation_id,
                            "image",
                            occurred_at.to_rfc3339(),
                            crate::prompt::IMAGE_SENSATION_TEXT,
                        ),
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
                        sensation_node(
                            &sensation_id,
                            "geolocation",
                            occurred_at.to_rfc3339(),
                            &geolocation_how(loc),
                        ),
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
        } else if let Some(motion) = payload.downcast_ref::<BrowserMotion>() {
            let id = browser_motion_content_id(motion);
            let sensation_id = sensation_id("browser_motion", &id, occurred_at.to_rfc3339());
            self.store_once(
                format!("browser-motion:{id}"),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(
                            &sensation_id,
                            "browser_motion",
                            occurred_at.to_rfc3339(),
                            &browser_motion_how(motion),
                        ),
                        browser_motion_node(motion, &id, occurred_at.to_rfc3339()),
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
            let sensation_id = sensation_id(
                "geolocation_embedding",
                &geo_embedding.geoloc_id,
                occurred_at.to_rfc3339(),
            );
            self.store_once(
                format!("geolocation_embedding:{}", geo_embedding.geoloc_id),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(
                            &sensation_id,
                            "geolocation_embedding",
                            occurred_at.to_rfc3339(),
                            &geolocation_how(&geo_embedding.loc),
                        ),
                        geolocation_embedding_node(
                            &geo_embedding.loc,
                            &geo_embedding.geoloc_id,
                            occurred_at.to_rfc3339(),
                            &geo_embedding.embedding,
                            geo_embedding.vector_id.as_deref(),
                            geo_embedding.model.as_deref(),
                        ),
                    ],
                    "relationships": [
                        {
                            "from": sensation_id,
                            "to": geo_embedding.geoloc_id,
                            "type": "PRODUCED",
                        }
                    ],
                }),
            )
            .await;
        } else if let Some(image_embedding) = payload.downcast_ref::<ImageEmbedding>() {
            let sensation_id = sensation_id(
                "image_embedding",
                &image_embedding.image_id,
                occurred_at.to_rfc3339(),
            );
            self.store_once(
                format!("image_embedding:{}", image_embedding.image_id),
                json!({
                    "op": "merge_graph",
                    "nodes": [
                        sensation_node(
                            &sensation_id,
                            "image_embedding",
                            occurred_at.to_rfc3339(),
                            "I recognize the current camera frame visually.",
                        ),
                        image_embedding_node(
                            &image_embedding.image,
                            &image_embedding.image_id,
                            occurred_at.to_rfc3339(),
                            &image_embedding.embedding,
                            image_embedding.vector_id.as_deref(),
                            image_embedding.model.as_deref(),
                        )
                    ],
                    "relationships": [
                        {
                            "from": sensation_id,
                            "to": image_embedding.image_id,
                            "type": "PRODUCED",
                        }
                    ],
                }),
            )
            .await;
        } else {
            #[cfg(feature = "face")]
            if let Some(face) = payload.downcast_ref::<FaceInfo>() {
                let sensation_id = sensation_id("face", &face.face_id, occurred_at.to_rfc3339());
                self.store_once(
                    format!("face:{}", face.face_id),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(
                                &sensation_id,
                                "face",
                                occurred_at.to_rfc3339(),
                                &crate::prompt::face_count_sensation_text(1),
                            ),
                            {
                                "label": "Image",
                                "id": face.source_image_id,
                            },
                            face_node(face, occurred_at.to_rfc3339()),
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
                        sensation_node(
                            &sensation_id,
                            "audio",
                            occurred_at.to_rfc3339(),
                            &audio_how(audio),
                        ),
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
                            sensation_node(
                                &sensation_id,
                                "heartbeat",
                                occurred_at.to_rfc3339(),
                                "I feel a heartbeat.",
                            ),
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
                            sensation_node(
                                &sensation_id,
                                "object",
                                occurred_at.to_rfc3339(),
                                &object_how(object),
                            ),
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
            } else if let Some(value) = payload.downcast_ref::<Value>() {
                let id = json_sensation_id(value, occurred_at.to_rfc3339());
                let sensation_id = sensation_id("json", &id, occurred_at.to_rfc3339());
                self.store_once(
                    id.clone(),
                    json!({
                    "op": "merge_graph",
                    "nodes": [
                            sensation_node(
                                &sensation_id,
                                "json",
                                occurred_at.to_rfc3339(),
                                "I sense structured data.",
                            ),
                            json_sensation_node(value, &id, occurred_at.to_rfc3339()),
                        ],
                        "relationships": [{
                            "from": sensation_id,
                            "to": id,
                            "type": "OBSERVED",
                        }],
                    }),
                )
                .await;
            } else if let Some(impression) = payload.downcast_ref::<Impression<String>>() {
                let id = cognitive_sensation_payload_id(impression, occurred_at.to_rfc3339());
                let sensation_id = sensation_id("cognitive", &id, occurred_at.to_rfc3339());
                let mut sensation = sensation_node(
                    &sensation_id,
                    "cognitive",
                    occurred_at.to_rfc3339(),
                    &impression.summary,
                );
                sensation["how_formed_at"] = json!(impression.timestamp.to_rfc3339());
                sensation["source_sensation_ids"] = json!(impression.source_sensation_ids);
                let mut nodes = vec![sensation];
                let mut relationships = Vec::new();
                for source_id in &impression.source_sensation_ids {
                    nodes.push(source_sensation_ref_node(source_id));
                    relationships.push(json!({
                        "from": sensation_id,
                        "to": source_id,
                        "type": "DERIVED_FROM",
                    }));
                }
                self.store_once(
                    id.clone(),
                    json!({
                        "op": "merge_graph",
                        "nodes": nodes,
                        "relationships": relationships,
                    }),
                )
                .await;
            } else if let Some(context) = payload.downcast_ref::<WillContext>() {
                let occurred_at_str = occurred_at.to_rfc3339();
                let sensation_id = sensation_id("will_context", "current", occurred_at_str.clone());
                self.store_once(
                    format!("will_context:{}", occurred_at_str),
                    json!({
                        "op": "merge_graph",
                        "nodes": [
                            sensation_node(
                                &sensation_id,
                                "will_context",
                                occurred_at_str.clone(),
                                "I reflect on my current context and system prompt.",
                            ),
                            {
                                "label": "WillContext",
                                "id": format!("will_context:{}", occurred_at_str),
                                "data": serde_json::to_string(context).unwrap_or_default(),
                                "occurred_at": occurred_at_str,
                            }
                        ],
                        "relationships": [{
                            "from": sensation_id,
                            "to": format!("will_context:{}", occurred_at_str),
                            "type": "OBSERVED",
                        }],
                    }),
                )
                .await;
            } else if let Some(summary) = payload.downcast_ref::<CombobulationSummary>() {
                let id = combobulation_summary_id(summary, occurred_at.to_rfc3339());
                let sensation_id =
                    sensation_id("combobulation_summary", &id, occurred_at.to_rfc3339());
                let mut relationships = vec![json!({
                    "from": sensation_id,
                    "to": id,
                    "type": "OBSERVED",
                })];
                for source_id in &summary.source_sensation_ids {
                    relationships.push(json!({
                        "from": id,
                        "to": source_id,
                        "type": "DERIVED_FROM",
                    }));
                }
                self.store_once(
                    id.clone(),
                    json!({
                    "op": "merge_graph",
                    "nodes": [
                            sensation_node(
                                &sensation_id,
                                "combobulation_summary",
                                occurred_at.to_rfc3339(),
                                &summary.text,
                            ),
                            combobulation_summary_node(summary, &id, occurred_at.to_rfc3339()),
                        ],
                        "relationships": relationships,
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
    let sensation_id = sensation_id("voice", &voice.clip_id, occurred_at.clone());
    observer
        .store_once(
            format!("voice:{}", voice.clip_id),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(
                        &sensation_id,
                        "voice",
                        occurred_at.clone(),
                        "I hear a voice.",
                    ),
                    audio_embedding_node(
                        &voice.clip,
                        &voice.clip_id,
                        occurred_at,
                        &voice.embedding,
                        voice.vector_id.as_deref(),
                        voice.model.as_deref(),
                    ),
                ],
                "relationships": [
                    {
                        "from": sensation_id,
                        "to": voice.clip_id,
                        "type": "PRODUCED",
                    }
                ],
            }),
        )
        .await;
}

async fn store_textual_sensation(observer: &SensationGraphObserver, sensation: &Sensation) {
    let (kind, text, occurred_at) = match sensation {
        Sensation::HeardOwnVoice { text, occurred_at } => {
            return store_spoken_sensation(observer, "self", text, occurred_at).await;
        }
        Sensation::HeardUserVoice { text, occurred_at } => {
            return store_spoken_sensation(observer, "user", text, occurred_at).await;
        }
        Sensation::WebInterfaceText { text, occurred_at } => {
            ("web_interface_text", text, occurred_at)
        }
        Sensation::StartedSpeaking { text, occurred_at } => {
            return store_spoken_sensation(observer, "started_speaking", text, occurred_at).await;
        }
        Sensation::FinishedSpeaking { text, occurred_at } => {
            return store_spoken_sensation(observer, "finished_speaking", text, occurred_at).await;
        }
        Sensation::Of { .. } => return,
    };
    let id = format!("web-interface-text:{}:{text}", occurred_at.to_rfc3339());
    let sensation_id = sensation_id(kind, &id, occurred_at.to_rfc3339());
    observer
        .store_once(
            id.clone(),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(
                        &sensation_id,
                        kind,
                        occurred_at.to_rfc3339(),
                        &web_interface_text_how(text),
                    ),
                    {
                        "label": "WebInterfaceText",
                        "id": id,
                        "text": text,
                        "occurred_at": occurred_at.to_rfc3339(),
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

async fn store_spoken_sensation(
    observer: &SensationGraphObserver,
    speaker: &str,
    text: &str,
    occurred_at: &chrono::DateTime<chrono::Utc>,
) {
    let (speaker, text, occurred_at) = match speaker {
        "self" | "user" | "started_speaking" | "finished_speaking" => (speaker, text, occurred_at),
        _ => return,
    };
    let utterance_id = format!("utterance:{speaker}:{}:{text}", occurred_at.to_rfc3339());
    let sensation_id = sensation_id("utterance", &utterance_id, occurred_at.to_rfc3339());
    observer
        .store_once(
            format!("utterance:{speaker}:{}:{text}", occurred_at.to_rfc3339()),
            json!({
                "op": "merge_graph",
                "nodes": [
                    sensation_node(
                        &sensation_id,
                        "utterance",
                        occurred_at.to_rfc3339(),
                        &utterance_how(speaker, text),
                    ),
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

fn sensation_node(id: &str, kind: &str, occurred_at: String, how: &str) -> Value {
    json!({
        "label": "Sensation",
        "id": id,
        "kind": kind,
        "occurred_at": occurred_at,
        "how": first_person_present(how),
        "how_formed_at": chrono::Utc::now().to_rfc3339(),
    })
}

fn first_person_present(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "I sense something.".into();
    }
    if trimmed.ends_with('.') || trimmed.ends_with('!') || trimmed.ends_with('?') {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}

fn geolocation_how(loc: &GeoLoc) -> String {
    format!(
        "I feel I'm in the vicinity of latitude {:.5}, longitude {:.5}.",
        loc.latitude, loc.longitude
    )
}

fn browser_motion_how(motion: &BrowserMotion) -> String {
    if let Some(accel) = motion
        .acceleration
        .as_ref()
        .or(motion.acceleration_including_gravity.as_ref())
    {
        return format!(
            "I feel acceleration around x {:.2}, y {:.2}, z {:.2}.",
            accel.x.unwrap_or_default(),
            accel.y.unwrap_or_default(),
            accel.z.unwrap_or_default()
        );
    }
    if let Some(orientation) = &motion.orientation {
        return format!(
            "I feel orientation around alpha {:.1}, beta {:.1}, gamma {:.1}.",
            orientation.alpha.unwrap_or_default(),
            orientation.beta.unwrap_or_default(),
            orientation.gamma.unwrap_or_default()
        );
    }
    "I feel my device moving.".into()
}

fn audio_how(audio: &AudioClip) -> String {
    match audio
        .transcript
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        Some(transcript) => format!("I hear audio saying \"{}\".", transcript.trim()),
        None => "I'm listening.".into(),
    }
}

fn object_how(object: &ObjectInfo) -> String {
    match object
        .label
        .as_deref()
        .filter(|label| !label.trim().is_empty())
    {
        Some(label) => format!("I see a {label}."),
        None => "I see an object.".into(),
    }
}

fn utterance_how(speaker: &str, text: &str) -> String {
    match speaker {
        "self" => format!("I hear myself saying \"{}\".", text.trim()),
        "user" => format!("I hear the user saying \"{}\".", text.trim()),
        "started_speaking" => format!("I start saying \"{}\".", text.trim()),
        "finished_speaking" => format!("I finish saying \"{}\".", text.trim()),
        _ => format!("I hear someone saying \"{}\".", text.trim()),
    }
}

fn web_interface_text_how(text: &str) -> String {
    format!("I hear someone on my web interface type: {}", text.trim())
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
        "embedding": object.embedding,
        "embedding_len": object.embedding.len(),
        "embedding_kind": "object",
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

fn json_sensation_id(value: &Value, occurred_at: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.to_string().as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at.as_bytes());
    format!("json-sensation:sha256:{:x}", hasher.finalize())
}

fn json_sensation_node(value: &Value, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "JsonSensation",
        "id": id,
        "merge_key": "id",
        "value": value.clone(),
        "occurred_at": occurred_at,
    })
}

fn combobulation_summary_id(summary: &CombobulationSummary, occurred_at: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(summary.text.as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at.as_bytes());
    format!("combobulation-summary:sha256:{:x}", hasher.finalize())
}

fn cognitive_sensation_payload_id(impression: &Impression<String>, occurred_at: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(impression.summary.as_bytes());
    hasher.update([0]);
    hasher.update(impression.timestamp.to_rfc3339().as_bytes());
    hasher.update([0]);
    hasher.update(occurred_at.as_bytes());
    format!("cognitive-sensation:sha256:{:x}", hasher.finalize())
}

fn source_sensation_ref_node(source_id: &str) -> Value {
    json!({
        "label": "SourceSensationRef",
        "id": source_id,
    })
}

fn combobulation_summary_node(
    summary: &CombobulationSummary,
    id: &str,
    occurred_at: String,
) -> Value {
    json!({
        "label": "CombobulationSummary",
        "id": id,
        "text": summary.text,
        "summary": summary.text,
        "emoji": summary.emoji,
        "created_at": summary.created_at,
        "occurred_at": occurred_at,
        "source_sensation_ids": summary.source_sensation_ids,
    })
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
                        sensation_node(
                            &sensation_id,
                            "unknown",
                            occurred_at.clone(),
                            "I sense something.",
                        ),
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
        "label": "FaceInstance",
        "id": face.face_id,
        "source_image_id": face.source_image_id,
        "crop_mime": face.crop.mime.clone(),
        "crop_base64": face.crop.base64.clone(),
        "captured_at": face.crop.captured_at.clone(),
        "occurred_at": occurred_at,
        "embedding": face.embedding,
        "embedding_len": face.embedding.len(),
        "embedding_kind": "face_instance",
        "embedding_point_id": face.vector_id,
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

fn image_embedding_node(
    image: &ImageData,
    id: &str,
    occurred_at: String,
    embedding: &[f32],
    vector_id: Option<&str>,
    model: Option<&str>,
) -> Value {
    let mut node = image_node(image, id, occurred_at);
    node["embedding"] = json!(embedding);
    node["embedding_len"] = json!(embedding.len());
    node["embedding_kind"] = json!("image");
    node["embedding_point_id"] = json!(vector_id);
    node["embedding_model"] = json!(model);
    node
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
        "transcript": audio.transcript.clone(),
        "captured_at": audio.captured_at.clone(),
        "occurred_at": occurred_at,
    })
}

fn audio_embedding_node(
    audio: &AudioClip,
    id: &str,
    occurred_at: String,
    embedding: &[f32],
    vector_id: Option<&str>,
    model: Option<&str>,
) -> Value {
    let mut node = audio_node(audio, id, occurred_at);
    node["embedding"] = json!(embedding);
    node["embedding_len"] = json!(embedding.len());
    node["embedding_kind"] = json!("voice");
    node["embedding_point_id"] = json!(vector_id);
    node["embedding_model"] = json!(model);
    node
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

fn geolocation_embedding_node(
    loc: &GeoLoc,
    id: &str,
    occurred_at: String,
    embedding: &[f32],
    vector_id: Option<&str>,
    model: Option<&str>,
) -> Value {
    let mut node = geolocation_node(loc, id, occurred_at);
    node["embedding"] = json!(embedding);
    node["embedding_len"] = json!(embedding.len());
    node["embedding_kind"] = json!("geolocation");
    node["embedding_point_id"] = json!(vector_id);
    node["embedding_model"] = json!(model);
    node
}

fn browser_motion_node(motion: &BrowserMotion, id: &str, occurred_at: String) -> Value {
    json!({
        "label": "BrowserMotion",
        "id": id,
        "merge_key": "id",
        "acceleration": motion.acceleration.clone(),
        "acceleration_including_gravity": motion.acceleration_including_gravity.clone(),
        "rotation_rate": motion.rotation_rate.clone(),
        "orientation": motion.orientation.clone(),
        "interval": motion.interval,
        "observed_at": motion.observed_at.clone(),
        "occurred_at": occurred_at,
    })
}
