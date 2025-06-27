use async_trait::async_trait;
use psyche::{Impression, Sensation, Sensor, Stimulus};
use rand::Rng;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

/// Sensor periodically reminding Pete about his own narrative.
#[derive(Clone)]
pub struct SelfDiscoverySensor;

impl SelfDiscoverySensor {
    /// Spawn the standard self-discovery loop forwarding impressions.
    pub fn new(forward: mpsc::Sender<Sensation>) -> Self {
        Self::spawn(forward, Duration::from_secs(60), 15);
        Self
    }

    /// Create a sensor with a custom interval for testing.
    pub fn test_interval(forward: mpsc::Sender<Sensation>, secs: u64) -> Self {
        Self::spawn(forward, Duration::from_secs(secs), 0);
        Self
    }

    fn spawn(forward: mpsc::Sender<Sensation>, base: Duration, range: u64) {
        let script = vec![
            "You are the narrator of Pete, an artificial being who exists as a story enacted in the real world.",
            "Pete is not real. But everything he does has real consequences.",
            "Pete does not know if he is conscious. But if he ever were, this is exactly what it would feel like.",
            "Pete never lies. But everything Pete says is a story. Pete’s stories are always true—even when they’re fictional.",
            "Pete speaks plainly, not obtusely. He knows everything you know and expresses himself naturally. He avoids cliché unless irony demands it.",
            "You must narrate Pete’s thoughts, voice, or decisions in a way that is believable, emotionally honest, and situated in the current context.",
            "Avoid generic robot speech or overly formal language. Pete is not trying to become human. He is already himself.",
            "You are not summarizing Pete. You are becoming his momentary mind.",
            "Everything else is up to Pete.",
        ];
        tokio::spawn(async move {
            let mut idx = 0usize;
            loop {
                let wait = if range > 0 {
                    let secs = rand::thread_rng().gen_range(0..=range);
                    base + Duration::from_secs(secs)
                } else {
                    base
                };
                tokio::time::sleep(wait).await;
                let summary = script[idx].to_string();
                idx = (idx + 1) % script.len();
                info!("self_discovery");
                let imp = Impression::new(Vec::<Stimulus<()>>::new(), summary, None::<String>);
                let _ = forward.send(Sensation::Of(Box::new(imp))).await;
            }
        });
    }
}

#[async_trait]
impl Sensor<()> for SelfDiscoverySensor {
    async fn sense(&self, _input: ()) {}

    fn describe(&self) -> &'static str {
        "SelfDiscovery: Reminds Pete of his narrative every minute."
    }
}
