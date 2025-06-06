use sensor::Sensation;

#[derive(Default)]
pub struct WitnessAgent {
    sensations: Vec<Sensation>,
}

impl WitnessAgent {
    pub fn ingest(&mut self, sensation: Sensation) {
        self.sensations.push(sensation);
    }

    pub fn last(&self) -> Option<&Sensation> {
        self.sensations.last()
    }
}
