use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};

use crate::{Processor, Task, TaskKind, TaskOutput};

/// Wraps a [`Processor`] and records how long each task takes to complete.
///
/// # Example
/// ```
/// use lingproc::{profiling::ProfilingProcessor, Processor, Task, InstructionFollowingTask, TaskOutput, TaskKind};
/// use futures::{StreamExt, stream::BoxStream};
///
/// struct Echo;
///
/// #[async_trait::async_trait]
/// impl Processor for Echo {
///     fn capabilities(&self) -> Vec<TaskKind> { vec![TaskKind::InstructionFollowing] }
///     async fn process(&self, task: Task) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
///         match task {
///             Task::InstructionFollowing(t) => {
///                 use async_stream::stream;
///                 let instr = t.instruction.clone();
///                 let s = stream! { yield Ok(TaskOutput::TextChunk(instr)); };
///                 Ok(Box::pin(s))
///             }
///             _ => Err(anyhow::anyhow!("unsupported")),
///         }
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let proc = ProfilingProcessor::new(Echo);
/// let task = Task::InstructionFollowing(InstructionFollowingTask { instruction: "hi".into(), images: vec![] });
/// let mut s = proc.process(task).await.unwrap();
/// while let Some(_chunk) = s.next().await {}
/// assert_eq!(proc.durations().len(), 1);
/// # });
/// ```
pub struct ProfilingProcessor<P> {
    inner: P,
    durations: Arc<Mutex<Vec<Duration>>>,
}

impl<P> ProfilingProcessor<P> {
    /// Create a new profiling wrapper around `inner`.
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            durations: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Retrieve recorded durations.
    pub fn durations(&self) -> std::sync::MutexGuard<'_, Vec<Duration>> {
        self.durations.lock().unwrap()
    }
}

#[async_trait]
impl<P: Processor + Send + Sync> Processor for ProfilingProcessor<P> {
    fn capabilities(&self) -> Vec<TaskKind> {
        self.inner.capabilities()
    }

    async fn process(
        &self,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
        let start = Instant::now();
        let inner_stream = self.inner.process(task).await?;
        let durations = self.durations.clone();
        let s = async_stream::stream! {
            futures::pin_mut!(inner_stream);
            while let Some(item) = inner_stream.next().await {
                yield item;
            }
            let dur = start.elapsed();
            durations.lock().unwrap().push(dur);
        };
        Ok(Box::pin(s))
    }
}
