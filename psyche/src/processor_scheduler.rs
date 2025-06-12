use crate::{
    Experience, Scheduler, Sensation,
    bus::{Event, EventBus},
};
use std::sync::Arc;

/// Scheduler using an LLM processor to summarize experience text.
pub struct ProcessorScheduler<P> {
    pub(crate) processor: P,
    bus: Arc<EventBus>,
    name: String,
}

impl<P> ProcessorScheduler<P> {
    /// Create a new scheduler wrapping the given processor.
    pub fn new(processor: P, bus: Arc<EventBus>, name: impl Into<String>) -> Self {
        Self {
            processor,
            bus,
            name: name.into(),
        }
    }

    /// Capabilities advertised by the underlying processor.
    pub fn capabilities(&self) -> Vec<lingproc::TaskKind>
    where
        P: lingproc::Processor,
    {
        self.processor.capabilities()
    }
}

impl<P> Scheduler for ProcessorScheduler<P>
where
    P: lingproc::Processor + Send + Sync + 'static,
{
    type Output = String;

    fn schedule(&mut self, prompt: &str, batch: Vec<Experience>) -> Option<Sensation<String>> {
        use futures::StreamExt;
        use lingproc::{InstructionFollowingTask, Task, TaskOutput};

        if batch.is_empty() {
            return None;
        }

        log::info!("processor scheduler starting");

        self.bus.send(Event::ProcessorPrompt {
            name: self.name.clone(),
            prompt: prompt.to_string(),
        });
        log::info!("llm prompt: {}", prompt);
        drop(batch);

        let task = Task::InstructionFollowing(InstructionFollowingTask {
            instruction: prompt.to_string(),
            images: vec![],
        });

        let handle = tokio::runtime::Handle::current();
        let text =
            tokio::task::block_in_place(|| match handle.block_on(self.processor.process(task)) {
                Ok(mut stream) => {
                    let mut text = String::new();
                    while let Some(chunk) = handle.block_on(stream.next()) {
                        match chunk {
                            Ok(TaskOutput::TextChunk(t)) => {
                                log::info!("llm chunk: {}", t);
                                self.bus.send(Event::ProcessorChunk {
                                    name: self.name.clone(),
                                    chunk: t.clone(),
                                });
                                text.push_str(&t);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("processor stream error: {e}");
                                return None;
                            }
                        }
                    }
                    Some(text)
                }
                Err(e) => {
                    log::error!("processor execution error: {e}");
                    None
                }
            })?;
        log::info!("processor scheduler finished");
        Some(Sensation::new(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Experience, Sensation, Sensor, Wit, bus::EventBus};
    use async_stream::stream;
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use lingproc::{Processor, Task, TaskKind, TaskOutput};
    use std::sync::Arc;

    struct MockProcessor;

    #[async_trait]
    impl Processor for MockProcessor {
        fn capabilities(&self) -> Vec<TaskKind> {
            vec![TaskKind::InstructionFollowing]
        }

        async fn process(
            &self,
            task: Task,
        ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
            match task {
                Task::InstructionFollowing(t) => {
                    let instr = t.instruction;
                    let s =
                        stream! { yield Ok(TaskOutput::TextChunk(format!("processed {instr}"))); };
                    Ok(Box::pin(s))
                }
                _ => Err(anyhow::anyhow!("unsupported")),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn processor_scheduler_runs_llm() {
        let bus = Arc::new(EventBus::new());
        let scheduler = ProcessorScheduler::new(MockProcessor, bus, "mock");
        let mut wit = Wit::with_config(scheduler, None, "mock");
        wit.feel(Sensation::new(Experience::new("one")));
        wit.feel(Sensation::new(Experience::new("two")));
        let exp = wit.tick().unwrap();
        assert!(exp.how.starts_with("processed"));
        assert!(wit.memory.all()[0].what.starts_with("processed"));
    }

    struct FailProcessor;

    #[async_trait]
    impl Processor for FailProcessor {
        fn capabilities(&self) -> Vec<TaskKind> {
            vec![TaskKind::InstructionFollowing]
        }

        async fn process(
            &self,
            _task: Task,
        ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
            Err(anyhow::anyhow!("boom"))
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn processor_scheduler_handles_errors() {
        let bus = Arc::new(EventBus::new());
        let scheduler = ProcessorScheduler::new(FailProcessor, bus, "fail");
        let mut wit = Wit::with_config(scheduler, None, "fail");
        wit.feel(Sensation::new(Experience::new("one")));
        assert!(wit.tick().is_none());
        assert!(wit.memory.all().is_empty());
    }
}
