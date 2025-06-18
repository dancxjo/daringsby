use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Instruction, Message, Vectorizer};
use psyche::{Ear, Event, Mouth, Psyche, Sensation};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
struct Dummy {
    speaking: std::sync::Arc<AtomicBool>,
}

#[tokio::test]
async fn no_empty_stream_chunks() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Default)]
    struct BlankFirstLLM;

    #[async_trait]
    impl Doer for BlankFirstLLM {
        async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for BlankFirstLLM {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
            let stream = tokio_stream::iter(vec![Ok("  ".to_string()), Ok("hello".to_string())]);
            Ok(Box::pin(stream))
        }
    }

    #[async_trait]
    impl Vectorizer for BlankFirstLLM {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }

    #[derive(Clone, Default)]
    struct NullMouth(Arc<AtomicUsize>);

    #[async_trait]
    impl Mouth for NullMouth {
        async fn speak(&self, _text: &str) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        async fn interrupt(&self) {}
        fn speaking(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Default)]
    struct NullEar;

    #[async_trait]
    impl Ear for NullEar {
        async fn hear_self_say(&self, _t: &str) {}
        async fn hear_user_say(&self, _t: &str) {}
    }

    let mouth_count = Arc::new(AtomicUsize::new(0));
    let mouth = Arc::new(NullMouth(mouth_count.clone())) as Arc<dyn Mouth>;
    let ear = Arc::new(NullEar) as Arc<dyn Ear>;

    let mut psyche = Psyche::new(
        Box::new(BlankFirstLLM),
        Box::new(BlankFirstLLM),
        Box::new(BlankFirstLLM),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();

    let handle = tokio::spawn(async move { psyche.run().await });

    let mut got_empty_chunk = false;
    let mut got_non_empty_chunk = false;
    while let Ok(evt) = events.recv().await {
        match evt {
            Event::StreamChunk(chunk) => {
                if chunk.trim().is_empty() {
                    got_empty_chunk = true;
                } else {
                    got_non_empty_chunk = true;
                }
            }
            Event::IntentionToSay(msg) => {
                handle.abort();
                break;
            }
            _ => {}
        }
    }

    let _ = handle.await;

    assert!(got_non_empty_chunk);
    assert!(!got_empty_chunk);
    assert_eq!(mouth_count.load(Ordering::SeqCst), 1);
}

#[async_trait]
impl Mouth for Dummy {
    async fn speak(&self, _t: &str) {
        self.speaking.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Ear for Dummy {
    async fn hear_self_say(&self, _t: &str) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    async fn hear_user_say(&self, _t: &str) {}
}

#[async_trait]
impl Doer for Dummy {
    async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
        Ok("ok".into())
    }
}

#[async_trait]
impl Chatter for Dummy {
    async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(tokio_stream::once(Ok("hello world".to_string()))))
    }
}

#[async_trait]
impl Vectorizer for Dummy {
    async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0])
    }
}

#[tokio::test]
async fn waits_for_user_when_configured() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct CountingChatter {
        calls: std::sync::Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Chatter for CountingChatter {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
        }
    }

    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let chatter = CountingChatter::default();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(chatter.clone()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_speak_when_spoken_to(true);

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert_eq!(chatter.calls.load(Ordering::SeqCst), 0);

    input
        .send(Sensation::HeardUserVoice("hello".into()))
        .unwrap();

    while let Ok(evt) = events.recv().await {
        if let Event::IntentionToSay(msg) = evt {
            input.send(Sensation::HeardOwnVoice(msg)).unwrap();
            break;
        }
    }

    let psyche = handle.await.unwrap();
    assert_eq!(chatter.calls.load(Ordering::SeqCst), 1);
    assert!(!psyche.speaking());
}

#[tokio::test]
async fn adds_message_after_voice_heard() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    let mut saw_chunk = false;
    while let Ok(evt) = events.recv().await {
        match evt {
            Event::StreamChunk(_) => saw_chunk = true,
            Event::IntentionToSay(msg) => {
                input.send(Sensation::HeardOwnVoice(msg)).unwrap();
                break;
            }
            Event::SpeechAudio(_) => {}
            Event::EmotionChanged(_) => {}
        }
    }

    let psyche = handle.await.unwrap();
    assert!(saw_chunk);
    let conv = psyche.conversation();
    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}

#[tokio::test]
async fn interrupts_when_user_speaks() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth.clone(),
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    while let Ok(evt) = events.recv().await {
        if let Event::IntentionToSay(msg) = evt {
            assert!(mouth.speaking());
            input.send(Sensation::HeardUserVoice("hi".into())).unwrap();
            input.send(Sensation::HeardOwnVoice(msg)).unwrap();
            break;
        }
    }

    let psyche = handle.await.unwrap();
    assert!(!mouth.speaking());
    assert!(!psyche.speaking());
}

#[tokio::test]
async fn times_out_without_echo() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");
    psyche.set_echo_timeout(std::time::Duration::from_millis(10));

    let handle = tokio::spawn(async move { psyche.run().await });
    let psyche = handle.await.unwrap();
    let conv = psyche.conversation();
    let log_len = { conv.lock().await.all().len() };
    assert_eq!(log_len, 1);
}

#[tokio::test]
async fn speaking_flag_clears_after_echo() {
    let mouth = std::sync::Arc::new(Dummy::default());
    let ear = mouth.clone();
    let mut psyche = Psyche::new(
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        Box::new(Dummy::default()),
        std::sync::Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();
    let input = psyche.input_sender();

    let handle = tokio::spawn(async move { psyche.run().await });

    while let Ok(evt) = events.recv().await {
        if let Event::IntentionToSay(msg) = evt {
            input.send(Sensation::HeardOwnVoice(msg)).unwrap();
            break;
        }
    }

    let psyche = handle.await.unwrap();
    assert!(!psyche.speaking());
}

#[tokio::test]
async fn empty_response_does_not_trigger_echo_timeout() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Default)]
    struct SilentLLM;

    #[async_trait]
    impl Doer for SilentLLM {
        async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for SilentLLM {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
            Ok(Box::pin(tokio_stream::once(Ok(String::new()))))
        }
    }

    #[async_trait]
    impl Vectorizer for SilentLLM {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }

    #[derive(Clone, Default)]
    struct CountingMouth(Arc<AtomicUsize>);

    #[async_trait]
    impl Mouth for CountingMouth {
        async fn speak(&self, _text: &str) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        async fn interrupt(&self) {}
        fn speaking(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Default)]
    struct NullEar;

    #[async_trait]
    impl Ear for NullEar {
        async fn hear_self_say(&self, _t: &str) {}
        async fn hear_user_say(&self, _t: &str) {}
    }

    let mouth_count = Arc::new(AtomicUsize::new(0));
    let mouth = Arc::new(CountingMouth(mouth_count.clone())) as Arc<dyn Mouth>;
    let ear = Arc::new(NullEar) as Arc<dyn Ear>;

    let mut psyche = Psyche::new(
        Box::new(SilentLLM),
        Box::new(SilentLLM),
        Box::new(SilentLLM),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let handle = tokio::spawn(async move { psyche.run().await });

    let psyche = handle.await.unwrap();
    assert_eq!(mouth_count.load(Ordering::SeqCst), 0);
    assert!(!psyche.speaking());
    let conv_len = { psyche.conversation().lock().await.all().len() };
    assert_eq!(conv_len, 0);
}

#[tokio::test]
async fn no_intention_event_for_empty_response() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Default)]
    struct SilentLLM;

    #[async_trait]
    impl Doer for SilentLLM {
        async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for SilentLLM {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
            Ok(Box::pin(tokio_stream::once(Ok(String::new()))))
        }
    }

    #[async_trait]
    impl Vectorizer for SilentLLM {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }

    #[derive(Clone, Default)]
    struct NullMouth(Arc<AtomicUsize>);

    #[async_trait]
    impl Mouth for NullMouth {
        async fn speak(&self, _text: &str) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        async fn interrupt(&self) {}
        fn speaking(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Default)]
    struct NullEar;

    #[async_trait]
    impl Ear for NullEar {
        async fn hear_self_say(&self, _t: &str) {}
        async fn hear_user_say(&self, _t: &str) {}
    }

    let mouth_count = Arc::new(AtomicUsize::new(0));
    let mouth = Arc::new(NullMouth(mouth_count.clone())) as Arc<dyn Mouth>;
    let ear = Arc::new(NullEar) as Arc<dyn Ear>;

    let mut psyche = Psyche::new(
        Box::new(SilentLLM),
        Box::new(SilentLLM),
        Box::new(SilentLLM),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    psyche.set_turn_limit(1);
    psyche.set_system_prompt("sys");

    let mut events = psyche.subscribe();

    let handle = tokio::spawn(async move { psyche.run().await });
    let psyche = handle.await.unwrap();

    while let Ok(evt) = events.try_recv() {
        if let Event::IntentionToSay(msg) = evt {
            panic!("should not intend to say: {:?}", msg);
        }
    }

    assert_eq!(mouth_count.load(Ordering::SeqCst), 0);
    assert!(!psyche.speaking());
}
