use async_trait::async_trait;
use psyche_core::{Impression, NoopMemory, Psyche, Stimulus, Wit};
use std::sync::atomic::{AtomicUsize, Ordering};

struct EchoWit {
    count: AtomicUsize,
}

#[async_trait]
impl Wit for EchoWit {
    async fn tick(&mut self, inputs: Vec<Stimulus>) -> Option<Impression> {
        if self.count.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(Impression {
                stimuli: inputs,
                summary: "echo".into(),
                emoji: Some("🙂".into()),
                timestamp: 0,
            })
        } else {
            None
        }
    }

    fn name(&self) -> &'static str {
        "echo"
    }
}

#[tokio::test]
async fn psyche_recurses() {
    let mut psyche = Psyche {
        wits: vec![Box::new(EchoWit {
            count: AtomicUsize::new(0),
        })],
        stimuli: vec![Stimulus {
            what: serde_json::json!({"msg": "hi"}),
            timestamp: 0,
        }],
        memory: Some(Box::new(NoopMemory)),
    };

    psyche.tick().await;
    assert_eq!(psyche.stimuli.len(), 2);
}
