use pete::dummy_psyche;

#[tokio::test]
async fn dummy_psyche_initial_state() {
    let psyche = dummy_psyche();
    assert_eq!(psyche.conversation().lock().await.all().len(), 0);
    assert!(!psyche.speaking());
    assert_eq!(psyche.system_prompt(), psyche::DEFAULT_SYSTEM_PROMPT);
}
