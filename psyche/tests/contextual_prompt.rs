use psyche::prompt::PromptBuilder;
use psyche::topics::{Topic, TopicBus};
use psyche::{ContextualPrompt, Impression, Stimulus};
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn includes_all_context() {
    let bus = TopicBus::new(8);
    let prompt = ContextualPrompt::new(bus.clone());
    sleep(Duration::from_millis(100)).await;
    bus.publish(Topic::Identity, "I am Pete".to_string());
    sleep(Duration::from_millis(10)).await;
    bus.publish(
        Topic::Situation,
        Impression::new(
            vec![Stimulus::new("s".to_string())],
            "on porch",
            None::<String>,
        ),
    );
    sleep(Duration::from_millis(10)).await;
    bus.publish(
        Topic::Moment,
        Impression::new(
            vec![Stimulus::new("m".to_string())],
            "wind blew",
            None::<String>,
        ),
    );
    sleep(Duration::from_millis(10)).await;
    bus.publish(
        Topic::Instant,
        Impression::new(
            vec![Stimulus::new("i".to_string())],
            "user said hi",
            None::<String>,
        ),
    );
    sleep(Duration::from_millis(50)).await;
    let out = prompt.build("hi");
    println!("{}", out);
    assert!(out.contains("Identity: I am Pete"));
    assert!(out.contains("Situation: on porch"));
    assert!(out.contains("Moment: wind blew"));
    assert!(out.contains("Instant: user said hi"));
}

#[tokio::test]
async fn missing_context_is_empty() {
    let bus = TopicBus::new(8);
    let prompt = ContextualPrompt::new(bus.clone());
    sleep(Duration::from_millis(20)).await;
    bus.publish(Topic::Identity, "Pete".to_string());
    sleep(Duration::from_millis(20)).await;
    let out = prompt.build("hi");
    println!("{}", out);
    assert!(out.contains("Identity: Pete"));
    assert!(out.contains("Situation: "));
    assert!(out.contains("Moment: "));
    assert!(out.contains("Instant: "));
}
