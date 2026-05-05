use async_trait::async_trait;
use chrono::Utc;
use psyche::{
    GeoEmbedding, GeoLoc, QdrantClient, Sensation, Sensor, Topic, TopicBus, geoloc_content_id,
    geoloc_observed_at, geoloc_vector,
};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Sensor forwarding geolocation updates to the psyche.
#[derive(Clone)]
pub struct GeoSensor {
    forward: mpsc::Sender<Sensation>,
    vector_store: Option<(QdrantClient, TopicBus)>,
}

impl GeoSensor {
    /// Create a new `GeoSensor` using the provided channel.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self {
            forward,
            vector_store: None,
        }
    }

    /// Create a new `GeoSensor` that also stores geolocation vectors.
    pub fn with_vector_store(
        forward: mpsc::Sender<Sensation>,
        qdrant: QdrantClient,
        bus: TopicBus,
    ) -> Self {
        Self {
            forward,
            vector_store: Some((qdrant, bus)),
        }
    }
}

#[async_trait]
impl Sensor<GeoLoc> for GeoSensor {
    async fn sense(&self, mut loc: GeoLoc) {
        info!("geo sensor received location");
        debug!("geo sensor received location");
        let occurred_at = geoloc_observed_at(&loc).unwrap_or_else(Utc::now);
        if loc.observed_at.is_none() {
            loc.observed_at = Some(occurred_at.to_rfc3339());
        }
        match self
            .forward
            .try_send(Sensation::of_at(loc.clone(), occurred_at))
        {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("dropping geolocation update because psyche input is full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!("dropping geolocation update because psyche input is closed");
            }
        }
        if let Some((qdrant, bus)) = &self.vector_store {
            let geoloc_id = geoloc_content_id(&loc);
            let embedding = geoloc_vector(&loc);
            let vector_id = match qdrant
                .store_geolocation_vector_for(&geoloc_id, loc.latitude, loc.longitude, &embedding)
                .await
            {
                Ok(id) => Some(id.to_string()),
                Err(e) => {
                    warn!(?e, "failed storing geolocation vector");
                    None
                }
            };
            bus.publish(
                Topic::Sensation,
                Sensation::of_at(
                    GeoEmbedding {
                        loc,
                        geoloc_id,
                        embedding,
                        vector_id,
                        model: Some("earth-unit-sphere/v1".to_string()),
                    },
                    occurred_at,
                ),
            );
        }
    }

    fn describe(&self) -> &'static str {
        "You know where you are in terms of latitude and longitude. This may \
help you remember where events happened."
    }
}
