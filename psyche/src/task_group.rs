/// Collection of background tasks that abort on drop.
///
/// `TaskGroup` owns a set of spawned tasks and ensures they
/// are cancelled when dropped. Call [`shutdown`] to wait for
/// all tasks to finish cancelling.
///
/// ```
/// use psyche::TaskGroup;
/// use tokio::runtime::Runtime;
/// use tokio::time::{sleep, Duration};
///
/// let rt = Runtime::new().unwrap();
/// rt.block_on(async {
///     let mut group = TaskGroup::new();
///     group.spawn(async { sleep(Duration::from_millis(10)).await; });
///     group.shutdown().await;
/// });
/// ```
pub struct TaskGroup {
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl TaskGroup {
    pub fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    pub fn spawn<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.handles.push(tokio::spawn(fut));
    }

    pub async fn shutdown(mut self) {
        for h in &self.handles {
            h.abort();
        }
        for h in self.handles.drain(..) {
            let _ = h.await;
        }
    }
}

impl Drop for TaskGroup {
    fn drop(&mut self) {
        for h in &self.handles {
            h.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TaskGroup;

    #[tokio::test]
    async fn shutdown_waits_for_tasks() {
        let mut group = TaskGroup::new();
        group.spawn(async { tokio::time::sleep(std::time::Duration::from_millis(1)).await });
        group.shutdown().await;
    }

    #[tokio::test]
    async fn drop_aborts_tasks() {
        let mut group = TaskGroup::new();
        group.spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });
        drop(group); // should abort without waiting a second
    }
}
