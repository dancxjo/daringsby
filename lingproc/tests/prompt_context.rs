use lingproc::{push_prompt_context, take_prompt_context};

#[tokio::test]
async fn notes_are_consumed_after_take() {
    // clear any leftover state
    take_prompt_context().await;
    push_prompt_context("alpha").await;
    push_prompt_context("beta").await;
    assert_eq!(
        take_prompt_context().await,
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert!(take_prompt_context().await.is_empty());
}
