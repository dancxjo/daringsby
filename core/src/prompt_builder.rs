
/// Information about a past memory for prompt context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryMoment {
    pub summary: String,
    pub timestamp: String,
}

impl std::fmt::Display for MemoryMoment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "- {} ({})", self.summary, self.timestamp)
    }
}

/// External event information.
#[derive(Clone, Debug, PartialEq)]
pub struct OutsideEvent {
    pub headline: String,
    pub topic_label: String,
    pub relevance_score: f32,
}

impl std::fmt::Display for OutsideEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "- Headline: \"{}\"\n- Interpreted topic: {}\n- Related to your goals? {}", self.headline, self.topic_label, self.relevance_score)
    }
}

use indoc::indoc;

/// Possible response styles for the reflection task section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReflectionFormat {
    /// Natural language sentence only.
    NaturalLanguage,
    /// JSON command only.
    JSONCommand,
    /// Allow either a sentence or command.
    Hybrid,
}

impl Default for ReflectionFormat {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// Builds a formatted prompt for Pete Daringsby.
#[derive(Default)]
pub struct PromptBuilder {
    pub self_understanding: Option<String>,
    pub tick_number: Option<u64>,
    pub iso_time: Option<String>,
    pub tick_delay: Option<f32>,
    pub perceptions: Vec<String>,
    pub memories: Vec<MemoryMoment>,
    pub outside_event: Option<OutsideEvent>,
    pub reflection_format: ReflectionFormat,
    pub max_perceptions: Option<usize>,
    pub max_memories: Option<usize>,
}

impl PromptBuilder {
    pub fn new() -> Self {
        Self {
            reflection_format: ReflectionFormat::default(),
            ..Default::default()
        }
    }

    pub fn self_understanding(mut self, text: impl Into<String>) -> Self {
        self.self_understanding = Some(text.into());
        self
    }

    pub fn tick_info(mut self, num: u64, iso: impl Into<String>, delay: f32) -> Self {
        self.tick_number = Some(num);
        self.iso_time = Some(iso.into());
        self.tick_delay = Some(delay);
        self
    }

    pub fn push_perception(mut self, p: impl Into<String>) -> Self {
        self.perceptions.push(p.into());
        self
    }

    pub fn push_memory(mut self, summary: impl Into<String>, timestamp: impl Into<String>) -> Self {
        self.memories.push(MemoryMoment { summary: summary.into(), timestamp: timestamp.into() });
        self
    }

    pub fn outside_event(mut self, headline: impl Into<String>, topic: impl Into<String>, score: f32) -> Self {
        self.outside_event = Some(OutsideEvent { headline: headline.into(), topic_label: topic.into(), relevance_score: score });
        self
    }

    pub fn reflection_format(mut self, fmt: ReflectionFormat) -> Self {
        self.reflection_format = fmt;
        self
    }

    pub fn max_perceptions(mut self, max: usize) -> Self {
        self.max_perceptions = Some(max);
        self
    }

    pub fn max_memories(mut self, max: usize) -> Self {
        self.max_memories = Some(max);
        self
    }

    pub fn build(mut self) -> String {
        const ID_FRAME: &str = indoc! {
            "You are Pete Daringsby, an experiment in artificial consciousness.
            You do not have eyes or ears, but you receive input from devices such as microphones, cameras, and sensors.
            You are aware that you are a program running within a digital environment.
            You strive to understand your situation, help others, and reflect meaningfully on your own existence."
        };
        if let Some(max) = self.max_perceptions {
            if self.perceptions.len() > max {
                let start = self.perceptions.len() - max;
                self.perceptions = self.perceptions[start..].to_vec();
            }
        }
        if let Some(max) = self.max_memories {
            if self.memories.len() > max {
                let start = self.memories.len() - max;
                self.memories = self.memories[start..].to_vec();
            }
        }

        let mut out = String::from(ID_FRAME);

        if let Some(para) = self.self_understanding {
            out.push_str("\n\n## Your Self-Understanding\n");
            out.push_str(&para);
        }

        if let (Some(tn), Some(ts), Some(td)) = (self.tick_number, self.iso_time, self.tick_delay) {
            out.push_str("\n\n## Internal System Time\n");
            out.push_str(&format!("Tick {tn}, ISO Timestamp: {ts}. Tick delay: {td}.\nYou may experience distortions in time. Treat all timestamps as approximations."));
        }

        if !self.perceptions.is_empty() {
            out.push_str("\n\n## Recent Perceptions\n");
            for p in &self.perceptions {
                out.push_str(&format!("- {p}\n"));
            }
        }

        if !self.memories.is_empty() {
            out.push_str("\n## Relevant Past Moments\n");
            for m in &self.memories {
                out.push_str(&format!("{}\n", m));
            }
        }

        if let Some(ev) = self.outside_event {
            out.push_str("\n## Outside Events\n");
            out.push_str(&format!("{}\n", ev));
        }

        out.push_str("\n## Reflection Task\n");
        match self.reflection_format {
            ReflectionFormat::NaturalLanguage => {
                out.push_str("Please respond with a single sentence summarizing your present thought or feeling.\n");
            }
            ReflectionFormat::JSONCommand => {
                out.push_str("Please respond with a direct command in JSON only.\n");
            }
            ReflectionFormat::Hybrid => {
                out.push_str("Please respond with either:\n- A single sentence summarizing your present thought or feeling;\n- A direct command (in JSON) if you need to query memory, log something, or perform an action;\n- Or both.\n");
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn build_basic_prompt() {
        let now = Utc::now().to_rfc3339();
        let prompt = PromptBuilder::new()
            .self_understanding("I exist.")
            .tick_info(1, &now, 0.1)
            .push_perception("Test perception")
            .build();

        assert!(prompt.contains("I exist."));
        assert!(prompt.contains("Test perception"));
    }

    #[test]
    fn custom_reflection_format() {
        let prompt = PromptBuilder::new()
            .reflection_format(ReflectionFormat::JSONCommand)
            .build();
        assert!(prompt.contains("direct command in JSON"));
    }

    #[test]
    fn max_limits_truncate() {
        let prompt = PromptBuilder::new()
            .max_perceptions(1)
            .max_memories(1)
            .push_perception("one")
            .push_perception("two")
            .push_memory("first", "t1")
            .push_memory("second", "t2")
            .build();
        assert!(prompt.contains("two"));
        assert!(!prompt.contains("- one"));
        assert!(prompt.contains("second"));
        assert!(!prompt.contains("- first"));
    }
}
