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
    assert!(prompt.contains("not the sensor stream"));
    assert!(prompt.contains("Pete's own vision, hearing, body sense, position sense"));
    assert!(prompt.contains("not as media files or external sensor artifacts"));
    assert!(prompt.contains("amount, density, cadence, or mix of input modalities"));
    assert!(prompt.contains("I cannot tell what is happening yet"));
    assert!(prompt.contains("sensor volume alone"));
    assert!(prompt.contains("Do not say that you are observing a timeline"));
    assert!(prompt.contains("Compress repeated or low-level records"));
    assert!(prompt.contains("do not enumerate ids"));
}

#[tokio::test]
async fn bus_backed_digest_loops_summary_back_as_sensation() {
    let bus = TopicBus::new(8);
    let sensation_bus = bus.clone();
    let sensations = sensation_bus.subscribe(Topic::Sensation);
    futures::pin_mut!(sensations);
    let combo = Combobulator::with_bus(bus, Arc::new(Dummy));
    let source_occurred_at = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    combo
        .digest(&[Impression::new(
            vec![Stimulus::with_source_sensation_ids(
                "I heard a voice nearby.".to_string(),
                source_occurred_at,
                ["sensation:audio:1"],
            )],
            "",
            None::<String>,
        )])
        .await
        .unwrap();

    let payload = sensations.next().await.unwrap();
    let sensation = payload.downcast_ref::<Sensation>().unwrap();
    let Sensation::Of {
        payload,
        occurred_at,
    } = sensation
    else {
        panic!("expected combobulation sensation");
    };
    assert_eq!(*occurred_at, source_occurred_at);
    let summary = payload.downcast_ref::<CombobulationSummary>().unwrap();
    assert_eq!(summary.text, "All clear.");
    assert_eq!(summary.source_sensation_ids, vec!["sensation:audio:1"]);
}
