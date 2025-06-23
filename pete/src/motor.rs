use async_trait::async_trait;
use psyche::traits::Motor;
use tracing::info;

/// Simple [`Motor`] implementation that logs each action.
#[derive(Clone, Default)]
pub struct LoggingMotor;

#[async_trait]
impl Motor for LoggingMotor {
    async fn say(&self, text: &str) {
        info!(%text, "motor say");
    }

    async fn set_emotion(&self, emoji: &str) {
        info!(%emoji, "motor set_emotion");
    }

    async fn take_photo(&self) {
        info!("motor take_photo");
    }

    async fn focus_on(&self, name: &str) {
        info!(%name, "motor focus_on");
    }
}
