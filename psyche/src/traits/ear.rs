use async_trait::async_trait;
/// All callbacks are made from the conversation loop.
#[async_trait]
pub trait Ear: Send + Sync {
    /// Notifies the ear that Pete spoke `text`.
    async fn hear_self_say(&self, text: &str);
    /// Notifies the ear that the user said `text`.
    async fn hear_user_say(&self, text: &str);
}
