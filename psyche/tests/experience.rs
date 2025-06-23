// // TODO Fix hung test

// use async_trait::async_trait;
// use lingproc::{Chatter, Doer, Instruction, Message, Vectorizer};
// use psyche::wit::Wit;
// use psyche::{Ear, Event, Impression, Mouth, Psyche, Sensation};
// use std::sync::{Arc, Mutex};
// use std::time::Duration;

// #[derive(Clone, Default)]
// struct Dummy {
//     speaking: Arc<std::sync::atomic::AtomicBool>,
// }

// #[async_trait]
// impl Mouth for Dummy {
//     async fn speak(&self, _t: &str) {
//         self.speaking
//             .store(true, std::sync::atomic::Ordering::SeqCst);
//     }
//     async fn interrupt(&self) {
//         self.speaking
//             .store(false, std::sync::atomic::Ordering::SeqCst);
//     }
//     fn speaking(&self) -> bool {
//         self.speaking.load(std::sync::atomic::Ordering::SeqCst)
//     }
// }

// #[async_trait]
// impl Ear for Dummy {
//     async fn hear_self_say(&self, _t: &str) {
//         self.speaking
//             .store(false, std::sync::atomic::Ordering::SeqCst);
//     }
//     async fn hear_user_say(&self, _t: &str) {}
// }

// #[async_trait]
// impl Doer for Dummy {
//     async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
//         Ok("ok".into())
//     }
// }

// #[async_trait]
// impl Chatter for Dummy {
//     async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<lingproc::ChatStream> {
//         Ok(Box::pin(tokio_stream::once(Ok("hello".to_string()))))
//     }
// }

// #[async_trait]
// impl Vectorizer for Dummy {
//     async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
//         Ok(vec![0.0])
//     }
// }

// struct CountingWit {
//     ticks: Arc<Mutex<usize>>,
// }

// #[async_trait]
// impl Wit<(), ()> for CountingWit {
//     async fn observe(&self, _: ()) {}
//     async fn tick(&self) -> Option<Impression<()>> {
//         let mut t = self.ticks.lock().unwrap();
//         *t += 1;
//         Some(Impression::new("t", None::<String>, ()))
//     }
// }

// #[tokio::test]
// async fn registered_wit_ticks() {
//     let mouth = Arc::new(Dummy::default());
//     let ear = mouth.clone();
//     let mut psyche = Psyche::new(
//         Box::new(Dummy::default()),
//         Box::new(Dummy::default()),
//         Box::new(Dummy::default()),
//         Arc::new(psyche::NoopMemory),
//         mouth,
//         ear,
//     );
//     psyche.set_turn_limit(1);
//     psyche.set_echo_timeout(Duration::from_millis(20));

//     let ticks = Arc::new(Mutex::new(0));
//     psyche.register_typed_wit(Arc::new(CountingWit {
//         ticks: ticks.clone(),
//     }));

//     let mut events = psyche.subscribe();
//     let input = psyche.input_sender();

//     let handle = tokio::spawn(async move { psyche.run().await });

//     while let Ok(evt) = events.recv().await {
//         if let Event::Speech { text, .. } = evt {
//             tokio::time::sleep(Duration::from_millis(30)).await;
//             input.send(Sensation::HeardOwnVoice(text)).unwrap();
//             break;
//         }
//     }

//     let _psyche = handle.await.unwrap();
//     assert!(*ticks.lock().unwrap() > 0);
// }
