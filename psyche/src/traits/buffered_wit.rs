use crate::Impression;
use async_trait::async_trait;
use std::sync::Mutex;

/// Trait for wits that simply buffer inputs and process them on `tick`.
#[async_trait]
pub trait BufferedWit: Send + Sync {
    /// Type of input collected in the buffer.
    type Input: Send;
    /// Type of impression produced on tick.
    type Output: Send;

    /// Mutable access to the internal buffer.
    fn buffer(&self) -> &Mutex<Vec<Self::Input>>;

    /// Convert drained items into impressions.
    async fn process_buffer(&self, items: Vec<Self::Input>) -> Vec<Impression<Self::Output>>;

    /// Short static label used for debug reporting.
    fn label(&self) -> &'static str;
}

#[async_trait]
impl<T> crate::traits::wit::Wit for T
where
    T: BufferedWit,
{
    type Input = <T as BufferedWit>::Input;
    type Output = <T as BufferedWit>::Output;

    async fn observe(&self, input: Self::Input) {
        self.buffer().lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let items = {
            let mut buf = self.buffer().lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            buf.drain(..).collect::<Vec<_>>()
        };
        self.process_buffer(items).await
    }

    fn debug_label(&self) -> &'static str {
        self.label()
    }
}
