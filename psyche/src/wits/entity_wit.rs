use crate::sensors::face::FaceInfo;
use crate::traits::wit::Wit;
use crate::types::ObjectInfo;
use crate::wits::memory::Memory;
use crate::wits::memory::QdrantClient;
use crate::{Impression, Sensation, Stimulus};
use async_trait::async_trait;
use lingproc::math::cosine_similarity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// In-memory embedding store used for tests.
#[derive(Default)]
pub struct InMemoryEmbeddingDb {
    vectors: Mutex<Vec<Vec<f32>>>,
}

#[async_trait]
/// Abstraction over a vector similarity store.
///
/// Implementations persist embeddings and provide nearest-neighbour search
/// capabilities. Vectors are identified by an implementation-defined index.
///
/// The trait is asynchronous so implementations may perform network I/O.
pub trait EmbeddingDb: Send + Sync {
    /// Search for the first embedding similar to `vector`.
    ///
    /// Returns the stored index when the best match exceeds `threshold` using
    /// cosine similarity, otherwise `None`.
    async fn search(&self, vector: &[f32], threshold: f32) -> Option<usize>;

    /// Insert `vector` into the database and return its unique index.
    async fn insert(&self, vector: Vec<f32>) -> usize;
}

#[async_trait]
impl EmbeddingDb for InMemoryEmbeddingDb {
    async fn search(&self, vector: &[f32], threshold: f32) -> Option<usize> {
        let store = self.vectors.lock().unwrap();
        store
            .iter()
            .position(|v| cosine_similarity(v, vector) > threshold)
    }

    async fn insert(&self, vector: Vec<f32>) -> usize {
        let mut store = self.vectors.lock().unwrap();
        store.push(vector);
        store.len() - 1
    }
}

#[async_trait]
impl EmbeddingDb for QdrantClient {
    async fn search(&self, _vector: &[f32], _threshold: f32) -> Option<usize> {
        None
    }

    async fn insert(&self, vector: Vec<f32>) -> usize {
        let _ = self.store_face_vector(&vector).await;
        0
    }
}

/// Person identity linked to a face and optionally a name.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Person {
    pub id: usize,
    pub name: Option<String>,
}

/// Identified object.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Object {
    pub id: usize,
    pub label: Option<String>,
}

/// Wit responsible for linking faces, names and objects.
pub struct EntityWit {
    memory: Arc<dyn Memory>,
    face_db: Arc<dyn EmbeddingDb>,
    object_db: Arc<dyn EmbeddingDb>,
    faces: Mutex<Vec<FaceInfo>>,
    names: Mutex<Vec<String>>,
    objects: Mutex<Vec<ObjectInfo>>,
    people: Mutex<HashMap<usize, Person>>,       // id -> person
    objects_seen: Mutex<HashMap<usize, Object>>, // id -> object
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl EntityWit {
    /// Debug label for this wit.
    pub const LABEL: &'static str = "Entity";

    /// Create a new `EntityWit`.
    pub fn new(
        memory: Arc<dyn Memory>,
        face_db: Arc<dyn EmbeddingDb>,
        object_db: Arc<dyn EmbeddingDb>,
    ) -> Self {
        Self {
            memory,
            face_db,
            object_db,
            faces: Mutex::new(Vec::new()),
            names: Mutex::new(Vec::new()),
            objects: Mutex::new(Vec::new()),
            people: Mutex::new(HashMap::new()),
            objects_seen: Mutex::new(HashMap::new()),
            tx: None,
        }
    }

    /// Create with debug reports.
    pub fn with_debug(
        memory: Arc<dyn Memory>,
        face_db: Arc<dyn EmbeddingDb>,
        object_db: Arc<dyn EmbeddingDb>,
        tx: broadcast::Sender<crate::WitReport>,
    ) -> Self {
        Self {
            tx: Some(tx),
            ..Self::new(memory, face_db, object_db)
        }
    }
}

#[async_trait]
impl crate::traits::wit::Wit for EntityWit {
    type Input = Sensation;
    type Output = String;

    async fn observe(&self, sensation: Self::Input) {
        match sensation {
            Sensation::HeardUserVoice(text) => {
                self.names.lock().unwrap().push(text);
            }
            Sensation::HeardOwnVoice(_) => {}
            Sensation::Of(any) => {
                if let Some(face) = any.downcast_ref::<FaceInfo>() {
                    self.faces.lock().unwrap().push(face.clone());
                } else if let Some(obj) = any.downcast_ref::<ObjectInfo>() {
                    self.objects.lock().unwrap().push(obj.clone());
                }
            }
        }
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let faces = { self.faces.lock().unwrap().drain(..).collect::<Vec<_>>() };
        let mut names = { self.names.lock().unwrap().drain(..).collect::<Vec<_>>() };
        let objects = { self.objects.lock().unwrap().drain(..).collect::<Vec<_>>() };
        let mut out = Vec::new();
        for face in faces {
            let id = if let Some(pid) = self.face_db.search(&face.embedding, 0.92).await {
                pid
            } else {
                let pid = self.face_db.insert(face.embedding.clone()).await;
                pid
            };
            let name = names.pop();
            if let Some(n) = name.clone() {
                self.people.lock().unwrap().insert(
                    id,
                    Person {
                        id,
                        name: Some(n.clone()),
                    },
                );
            }
            let summary = if let Some(ref n) = self
                .people
                .lock()
                .unwrap()
                .get(&id)
                .and_then(|p| p.name.clone())
            {
                format!("Saw {n} (#{id})")
            } else {
                format!("Saw person #{id}")
            };
            let stim = Stimulus::new(summary.clone());
            let imp = Impression::new(vec![stim], summary.clone(), None::<String>);
            let _ = self.memory.store_serializable(&imp).await;
            out.push(imp);
            if let Some(tx) = &self.tx {
                if crate::debug::debug_enabled(Self::LABEL).await {
                    let _ = tx.send(crate::WitReport {
                        name: Self::LABEL.into(),
                        prompt: "link".into(),
                        output: summary.clone(),
                    });
                }
            }
        }
        for n in names {
            let id = {
                let mut people = self.people.lock().unwrap();
                let id = people.len();
                people.insert(
                    id,
                    Person {
                        id,
                        name: Some(n.clone()),
                    },
                );
                id
            };
            let summary = format!("Heard {n} (#{id})");
            let stim = Stimulus::new(summary.clone());
            let imp = Impression::new(vec![stim], summary.clone(), None::<String>);
            let _ = self.memory.store_serializable(&imp).await;
            out.push(imp);
        }
        for obj in objects {
            let id = if let Some(oid) = self.object_db.search(&obj.embedding, 0.92).await {
                oid
            } else {
                self.object_db.insert(obj.embedding.clone()).await
            };
            if obj.label.is_some() {
                self.objects_seen.lock().unwrap().insert(
                    id,
                    Object {
                        id,
                        label: obj.label.clone(),
                    },
                );
            }
            let summary = if let Some(l) = obj.label.as_deref() {
                format!("Saw {l} (#{id})")
            } else {
                format!("Saw object #{id}")
            };
            let stim = Stimulus::new(summary.clone());
            let imp = Impression::new(vec![stim], summary.clone(), None::<String>);
            let _ = self.memory.store_serializable(&imp).await;
            out.push(imp);
        }
        out
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for EntityWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(sensation) = payload.downcast_ref::<Sensation>() {
            match sensation {
                Sensation::HeardUserVoice(t) => {
                    self.observe(Sensation::HeardUserVoice(t.clone())).await;
                }
                Sensation::HeardOwnVoice(t) => {
                    self.observe(Sensation::HeardOwnVoice(t.clone())).await;
                }
                Sensation::Of(any) => {
                    if let Some(face) = any.downcast_ref::<FaceInfo>() {
                        self.observe(Sensation::Of(Box::new(face.clone()))).await;
                    } else if let Some(obj) = any.downcast_ref::<ObjectInfo>() {
                        self.observe(Sensation::Of(Box::new(obj.clone()))).await;
                    }
                }
            }
        }
    }
}
