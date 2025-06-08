use crate::traits::{LLMAttribute, LLMCapability};

/// Specification for a unit of language processing work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinguisticTask {
    /// The prompt or text to process.
    pub prompt: String,
    /// Capabilities required of the language processor.
    pub capabilities: Vec<LLMCapability>,
    /// Optional preferred processor attribute.
    pub prefer: Option<LLMAttribute>,
    /// Whether this task can be dropped if all processors are busy.
    pub droppable: bool,
}

impl LinguisticTask {
    /// Create a new task with the given prompt and capabilities.
    pub fn new(prompt: impl Into<String>, capabilities: Vec<LLMCapability>) -> Self {
        Self { prompt: prompt.into(), capabilities, prefer: None, droppable: false }
    }

    /// Indicate a preferred attribute for scheduling.
    pub fn prefer_attribute(mut self, attr: LLMAttribute) -> Self {
        self.prefer = Some(attr);
        self
    }

    /// Mark the task as droppable when the queue is full.
    pub fn droppable(mut self, value: bool) -> Self {
        self.droppable = value;
        self
    }
}
