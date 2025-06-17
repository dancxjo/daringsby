use chrono::Utc;
use psyche::wit::{Instant, MomentWit};
use psyche::{Impression, Summarizer};
use uuid::Uuid;

#[tokio::test]
async fn synthesizes_moment_from_instants() {
    let wit = MomentWit::default();

    let input = vec![
        Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "Saw a dog".into(),
            details: Some("At 10:01, a golden retriever barked at Pete.".into()),
            raw_data: Instant {
                observation: "a dog barked".into(),
            },
        },
        Impression {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            headline: "Pete felt startled".into(),
            details: Some("At 10:02, Pete's posture stiffened.".into()),
            raw_data: Instant {
                observation: "Pete was startled".into(),
            },
        },
    ];

    let output = wit.digest(&input).await.unwrap();

    assert_eq!(output.raw_data.summary.contains("dog"), true);
    assert_eq!(output.raw_data.summary.contains("startled"), true);
}
