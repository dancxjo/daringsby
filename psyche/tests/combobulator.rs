use async_trait::async_trait;
use futures::StreamExt;
use lingproc::LlmInstruction;
use psyche::traits::Doer;
use psyche::{
    CombobulationSummary, Impression, Sensation, Stimulus, Topic, TopicBus, wits::Combobulator,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Dummy;

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: LlmInstruction) -> anyhow::Result<String> {
        Ok("All clear.".to_string())
    }
}

#[derive(Clone, Default)]
struct CapturingDoer {
    command: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl Doer for CapturingDoer {
    async fn follow(&self, instruction: LlmInstruction) -> anyhow::Result<String> {
        *self.command.lock().unwrap() = Some(instruction.command);
        Ok("All clear.".to_string())
    }
}

#[tokio::test]
async fn returns_awareness_impression() {
    let combo = Combobulator::new(Arc::new(Dummy));
    let imp = combo
        .digest(&[Impression::new(
            vec![Stimulus::new("Pete looked around.".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();
    assert_eq!(imp.stimuli[0].what, "All clear.");
    assert_eq!(imp.summary, "All clear.");
}

#[tokio::test]
async fn prompt_frames_inputs_as_real_world_events() {
    let doer = CapturingDoer::default();
    let captured = doer.command.clone();
    let combo = Combobulator::new(Arc::new(doer));

    combo
        .digest(&[Impression::new(
            vec![Stimulus::new(
                "SpeechSegment speech: possible thief".to_string(),
            )],
            "",
            None::<String>,
        )])
        .await
        .unwrap();

    let prompt = captured.lock().unwrap().clone().unwrap();
    assert!(prompt.contains("internal representations of sensations and real-world events"));
    assert!(prompt.contains("fragmentary, possibly contradictory, fleeting evidence"));
    assert!(prompt.contains("prior combobulation summaries looping back in as sensations"));
    assert!(prompt.contains("not as the topic to describe"));
    assert!(prompt.contains("audio recording and the transcription derived from it"));
    assert!(prompt.contains("Do not say that you are observing a timeline"));
    assert!(prompt.contains("Compress repeated or low-level records"));
    assert!(prompt.contains("do not enumerate ids"));
}

#[tokio::test]
async fn bus_backed_digest_loops_summary_back_as_sensation() {
    let bus = TopicBus::new(8);
    let mut sensations = bus.subscribe(Topic::Sensation);
    let combo = Combobulator::with_bus(bus, Arc::new(Dummy));

    combo
        .digest(&[Impression::new(
            vec![Stimulus::new("I heard a voice nearby.".to_string())],
            "",
            None::<String>,
        )])
        .await
        .unwrap();

    let payload = sensations.next().await.unwrap();
    let sensation = payload.downcast_ref::<Sensation>().unwrap();
    let Sensation::Of { payload, .. } = sensation else {
        panic!("expected combobulation sensation");
    };
    let summary = payload.downcast_ref::<CombobulationSummary>().unwrap();
    assert_eq!(summary.text, "All clear.");
}
