use std::thread;

/// The core AI engine.
///
/// Currently provides only a skeleton structure for experimentation.
#[derive(Debug, Default)]
pub struct Psyche;

impl Psyche {
    /// Construct a new [`Psyche`].
    pub fn new() -> Self {
        Self
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

    #[test]
    fn psyche_runs() {
        let psyche = Psyche::new();
        psyche.run();
    }
}
