use crate::{ModelRunnerProvider, Task, TaskOutput};
use futures::stream::BoxStream;

/// Simple task scheduler distributing work across providers.
///
/// Providers advertising the requested model are queried for their active
/// process count and run history. The scheduler then selects the provider with
/// the fewest active processes, falling back to the provider with the fewest
/// recorded runs on ties.
///
/// # Example
/// ```
/// use lingproc::{scheduler::Scheduler, provider::{ModelRunnerProvider, ProviderProfile}};
/// use lingproc::{Task, InstructionFollowingTask, Processor, TaskKind, TaskOutput};
/// use async_trait::async_trait;
/// use futures::{stream::BoxStream, StreamExt};
/// use std::sync::{Arc, Mutex};
///
/// struct EchoProvider {
///     profile: Arc<Mutex<ProviderProfile>>,
/// }
///
/// #[async_trait]
/// impl ModelRunnerProvider for EchoProvider {
///     async fn models(&self) -> anyhow::Result<Vec<modeldb::AiModel>> {
///         Ok(vec![modeldb::AiModel {
///             name: "echo".into(),
///             supports_images: false,
///             speed: None,
///             cost_per_token: None,
///             capabilities: vec![modeldb::Capability::InstructionFollowing],
///         }])
///     }
///     async fn processor_for(&self, _model: &str) -> anyhow::Result<Box<dyn Processor + Send + Sync>> {
///         struct Echo;
///         #[async_trait]
///         impl Processor for Echo {
///             fn capabilities(&self) -> Vec<TaskKind> { vec![TaskKind::InstructionFollowing] }
///             async fn process(&self, task: Task) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
///                 match task { Task::InstructionFollowing(t) => {
///                     use async_stream::stream;
///                     let s = stream! { yield Ok(TaskOutput::TextChunk(t.instruction)); };
///                     Ok(Box::pin(s))
///                 }, _ => unimplemented!() }
///             }
///         }
///         Ok(Box::new(Echo))
///     }
///     fn active(&self) -> usize { 0 }
///     async fn heartbeat(&self) -> bool { true }
///     async fn health_check(&self) -> bool { true }
///     fn profile(&self) -> ProviderProfile { self.profile.lock().unwrap().clone() }
/// }
///
/// # tokio_test::block_on(async {
/// let mut sched = Scheduler::new();
/// sched.add_provider(Box::new(EchoProvider { profile: Arc::new(Mutex::new(ProviderProfile::default())) }));
/// let task = Task::InstructionFollowing(InstructionFollowingTask { instruction: "hi".into(), images: vec![] });
/// let mut s = sched.run("echo", task).await.unwrap();
/// let first = s.next().await.unwrap().unwrap();
/// match first { TaskOutput::TextChunk(t) => assert_eq!(t, "hi"), _ => panic!("wrong") }
/// # });
/// ```
pub struct Scheduler {
    providers: Vec<Box<dyn ModelRunnerProvider + Send + Sync>>,
}

impl Scheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a new provider.
    pub fn add_provider(&mut self, provider: Box<dyn ModelRunnerProvider + Send + Sync>) {
        self.providers.push(provider);
    }

    async fn select_provider(&self, model: &str) -> anyhow::Result<&dyn ModelRunnerProvider> {
        let mut selected: Option<&Box<dyn ModelRunnerProvider + Send + Sync>> = None;
        let mut best_active = usize::MAX;
        let mut best_runs = usize::MAX;
        for p in &self.providers {
            let models = p.models().await?;
            if models.iter().any(|m| m.name == model) {
                let active = p.active();
                let runs = p.profile().runs(model);
                if active < best_active || (active == best_active && runs < best_runs) {
                    selected = Some(p);
                    best_active = active;
                    best_runs = runs;
                }
            }
        }
        selected
            .map(|p| &**p as &dyn ModelRunnerProvider)
            .ok_or_else(|| anyhow::anyhow!("no provider for model"))
    }

    /// Run a task on the best available provider.
    pub async fn run(
        &self,
        model: &str,
        task: Task,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>> {
        let provider = self.select_provider(model).await?;
        let proc = provider.processor_for(model).await?;
        proc.process(task).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Processor, ProviderProfile, TaskKind};
    use async_trait::async_trait;
    use futures::StreamExt;
    use std::sync::{Arc, Mutex};

    struct EchoProvider {
        profile: Arc<Mutex<ProviderProfile>>,
        active: Arc<Mutex<usize>>,
        name: String,
    }

    impl EchoProvider {
        fn new(name: &str) -> Self {
            Self {
                profile: Arc::new(Mutex::new(ProviderProfile::default())),
                active: Arc::new(Mutex::new(0)),
                name: name.into(),
            }
        }
    }

    #[async_trait]
    impl ModelRunnerProvider for EchoProvider {
        async fn models(&self) -> anyhow::Result<Vec<modeldb::AiModel>> {
            Ok(vec![modeldb::AiModel {
                name: self.name.clone(),
                supports_images: false,
                speed: None,
                cost_per_token: None,
                capabilities: vec![modeldb::Capability::InstructionFollowing],
            }])
        }

        async fn processor_for(
            &self,
            model: &str,
        ) -> anyhow::Result<Box<dyn Processor + Send + Sync>> {
            struct EchoProc {
                profile: Arc<Mutex<ProviderProfile>>,
                active: Arc<Mutex<usize>>,
                model: String,
            }

            #[async_trait]
            impl Processor for EchoProc {
                fn capabilities(&self) -> Vec<TaskKind> {
                    vec![TaskKind::InstructionFollowing]
                }

                async fn process(
                    &self,
                    task: Task,
                ) -> anyhow::Result<BoxStream<'static, anyhow::Result<TaskOutput>>>
                {
                    use async_stream::stream;
                    let profile = self.profile.clone();
                    let active = self.active.clone();
                    let model = self.model.clone();
                    match task {
                        Task::InstructionFollowing(t) => {
                            let s = stream! {
                                yield Ok(TaskOutput::TextChunk(t.instruction));
                                profile.lock().unwrap().record(&model);
                                *active.lock().unwrap() -= 1;
                            };
                            Ok(Box::pin(s))
                        }
                        _ => Err(anyhow::anyhow!("unsupported")),
                    }
                }
            }

            *self.active.lock().unwrap() += 1;
            Ok(Box::new(EchoProc {
                profile: self.profile.clone(),
                active: self.active.clone(),
                model: model.to_string(),
            }))
        }

        fn active(&self) -> usize {
            *self.active.lock().unwrap()
        }
        async fn heartbeat(&self) -> bool {
            true
        }
        async fn health_check(&self) -> bool {
            true
        }
        fn profile(&self) -> ProviderProfile {
            self.profile.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn distributes_by_load() {
        let p1 = EchoProvider::new("echo");
        let p2 = EchoProvider::new("echo");
        let mut sched = Scheduler::new();
        sched.add_provider(Box::new(p1));
        sched.add_provider(Box::new(p2));

        let task = Task::InstructionFollowing(crate::InstructionFollowingTask {
            instruction: "hi".into(),
            images: vec![],
        });
        let mut s1 = sched.run("echo", task.clone()).await.unwrap();
        while let Some(_c) = s1.next().await {}
        let mut s2 = sched.run("echo", task).await.unwrap();
        while let Some(_c) = s2.next().await {}

        let r1 = sched.providers[0].profile().runs("echo");
        let r2 = sched.providers[1].profile().runs("echo");
        assert_eq!(r1, 1);
        assert_eq!(r2, 1);
    }
}
