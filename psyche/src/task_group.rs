pub(crate) struct TaskGroup {
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl TaskGroup {
    pub(crate) fn new() -> Self {
        Self {
            handles: Vec::new(),
        }
    }

    pub(crate) fn spawn<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.handles.push(tokio::spawn(fut));
    }

    pub(crate) async fn shutdown(mut self) {
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
