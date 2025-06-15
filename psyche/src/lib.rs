pub mod ling;
use std::thread;
use ling::{Narrator, Voice, Vectorizer};

/// The core AI engine.
///
/// Currently provides only a skeleton structure for experimentation.
pub struct Psyche {
    narrator: Box<dyn Narrator>,
    voice: Box<dyn Voice>,
    vectorizer: Box<dyn Vectorizer>,
}

impl Psyche {
    /// Construct a new [`Psyche`].
    pub fn new(
        narrator: Box<dyn Narrator>,
        voice: Box<dyn Voice>,
        vectorizer: Box<dyn Vectorizer>,
    ) -> Self {
        Self { narrator, voice, vectorizer }
    }

    /// Spawn the conversation and experience threads and wait for them to finish.
    pub fn run(&self) {
        let converse_handle = thread::spawn(Self::converse);
        let experience_handle = thread::spawn(Self::experience);
        converse_handle.join().expect("converse thread panicked");
        experience_handle.join().expect("experience thread panicked");
    }

    fn converse() {
        // TODO: implement conversation loop
        println!("converse stub");
    }

    fn experience() {
        // TODO: implement experience processing
        println!("experience stub");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct Dummy;

    #[async_trait]
    impl Narrator for Dummy {
        async fn follow(&self, _: &str) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Voice for Dummy {
        async fn chat(&self, _: &str, _: &[ling::Message]) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![1.0])
        }
    }

    #[test]
    fn psyche_runs() {
        let psyche = Psyche::new(Box::new(Dummy), Box::new(Dummy), Box::new(Dummy));
        psyche.run();
    }
}
