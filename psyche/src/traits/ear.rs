use async_trait::async_trait;
use chrono::{DateTime, Utc};
/// All callbacks are made from the conversation loop.
#[async_trait]
pub trait Ear: Send + Sync {
    /// Notifies the ear that Pete spoke `text`.
    async fn hear_self_say(&self, text: &str);
    /// Notifies the ear that Pete spoke `text` at a known occurrence time.
    async fn hear_self_say_at(&self, text: &str, _occurred_at: DateTime<Utc>) {
        self.hear_self_say(text).await;
    }
    /// Notifies the ear that the user said `text`.
    async fn hear_user_say(&self, text: &str);
    /// Notifies the ear that the user said `text` at a known occurrence time.
    async fn hear_user_say_at(&self, text: &str, _occurred_at: DateTime<Utc>) {
        self.hear_user_say(text).await;
    }
}
