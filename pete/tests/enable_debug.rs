use pete::dummy_psyche;
use psyche::{debug_enabled, disable_debug};

#[tokio::test]
async fn enable_all_debug_turns_on_every_label() {
    let psyche = dummy_psyche();
    psyche.enable_all_debug().await;
    assert!(debug_enabled("Vision").await);
    disable_debug("Vision").await;
}
