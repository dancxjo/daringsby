use async_trait::async_trait;
use lingproc::LlmInstruction;
use psyche::traits::Doer;
use psyche::{Impression, Stimulus, wits::Combobulator};
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
    assert!(prompt.contains("internal representations of real-world events"));
    assert!(prompt.contains("not as the topic to describe"));
    assert!(prompt.contains("audio recording and the transcription derived from it"));
    assert!(prompt.contains("Do not say that you are observing a timeline"));
    assert!(prompt.contains("Compress repeated or low-level records"));
    assert!(prompt.contains("do not enumerate ids"));
}
