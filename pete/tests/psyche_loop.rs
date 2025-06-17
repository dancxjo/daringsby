use async_trait::async_trait;
use psyche::{Ear, Mouth, Sensation};
use std::sync::Arc;

/// Dummy mouth that records what was said.
struct TestMouth {
    spoken: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl Mouth for TestMouth {
    async fn speak(&self, text: &str) {
        self.spoken.lock().await.push(text.to_string());
    }
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool {
        false
    }
}

/// Dummy ear that records what was heard.
struct TestEar {
    heard_self: Arc<tokio::sync::Mutex<Vec<String>>>,
    heard_user: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl Ear for TestEar {
    async fn hear_self_say(&self, text: &str) {
        self.heard_self.lock().await.push(text.to_string());
    }

    async fn hear_user_say(&self, text: &str) {
        self.heard_user.lock().await.push(text.to_string());
    }
}

#[tokio::test]
async fn test_speak_and_echo_loop() {
    let spoken = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let heard_self = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let heard_user = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    let mouth = Arc::new(TestMouth {
        spoken: spoken.clone(),
    });
    let ear = Arc::new(TestEar {
        heard_self: heard_self.clone(),
        heard_user: heard_user.clone(),
    });

    let mut psyche = test_psyche(mouth.clone(), ear.clone());
    psyche.set_speak_when_spoken_to(true);
    let sender = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    sender
        .send(Sensation::HeardUserVoice("Hello there".into()))
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    sender.send(Sensation::HeardOwnVoice("Hi".into())).unwrap();
    handle.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let said = spoken.lock().await.clone();
    let heard = heard_self.lock().await.clone();
    let heard_u = heard_user.lock().await.clone();

    assert!(said.iter().any(|s| s.contains("Hi")));
    assert!(heard.iter().any(|s| s.contains("Hi")));
    assert!(heard_u.iter().any(|s| s.contains("Hello")));
}

#[tokio::test]
async fn test_countenance_sets_emotion() {
    use psyche::Countenance;
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

    impl Countenance for Recorder {
        fn express(&self, emoji: &str) {
            self.0.lock().unwrap().push(emoji.to_string());
        }
    }

    let recorder = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut psyche = test_psyche(
        Arc::new(TestMouth {
            spoken: Default::default(),
        }),
        Arc::new(TestEar {
            heard_self: Default::default(),
            heard_user: Default::default(),
        }),
    );
    psyche.set_countenance(Arc::new(Recorder(recorder.clone())));
    psyche.set_emotion("😊");

    let log = recorder.lock().unwrap().clone();
    assert_eq!(log, vec!["😊"]);
}

fn test_psyche(mouth: Arc<dyn Mouth>, ear: Arc<dyn Ear>) -> psyche::Psyche {
    use futures::stream;
    use psyche::ling::{ChatStream, Chatter, Doer, Instruction, Message, Vectorizer};
    use std::pin::Pin;

    struct DummyLLM;

    #[async_trait]
    impl Doer for DummyLLM {
        async fn follow(&self, _instruction: Instruction) -> anyhow::Result<String> {
            Ok("Done".into())
        }
    }

    #[async_trait]
    impl Chatter for DummyLLM {
        async fn chat(
            &self,
            _system_prompt: &str,
            _history: &[Message],
        ) -> anyhow::Result<ChatStream> {
            Ok(Box::pin(stream::iter(vec![Ok("Hi".into())])))
        }
    }

    #[async_trait]
    impl Vectorizer for DummyLLM {
        async fn vectorize(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    psyche::Psyche::new(
        Box::new(DummyLLM),
        Box::new(DummyLLM),
        Box::new(DummyLLM),
        mouth,
        ear,
    )
}
