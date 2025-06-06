use sensor::Sensation;

#[derive(Default)]
pub struct WitnessAgent {
    sensations: Vec<Sensation>,
}

impl WitnessAgent {
    pub fn ingest(&mut self, sensation: Sensation) {
        self.sensations.push(sensation);
    }

    pub fn last_text(&self) -> Option<&str> {
        self.sensations.last().map(|s| s.text.as_str())
    }
}
