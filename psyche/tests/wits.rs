use chrono::Utc;
use psyche::wit::{EpisodeWit, Instant, InstantWit, MomentWit, SituationWit};
use psyche::{Impression, Sensation, Summarizer};
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

#[tokio::test]
async fn instant_to_episode_pipeline() {
    let instant_wit = InstantWit::default();
    let moment_wit = MomentWit::default();
    let situation_wit = SituationWit::default();
    let episode_wit = EpisodeWit::default();

    let sensation = Impression::new(
        "",
        None::<String>,
        Sensation::HeardUserVoice("hello".into()),
    );
    let instant = instant_wit.digest(&[sensation]).await.unwrap();
    let moment = moment_wit.digest(&[instant.clone()]).await.unwrap();
    let situation = situation_wit.digest(&[moment.clone()]).await.unwrap();
    let episode = episode_wit.digest(&[situation]).await.unwrap();
    assert!(!episode.raw_data.summary.is_empty());
}
