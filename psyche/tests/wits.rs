use psyche::wit::{EpisodeWit, Instant, InstantWit, MomentWit, SituationWit};
use psyche::{Impression, Sensation, Stimulus, Summarizer};

#[tokio::test]
async fn synthesizes_moment_from_instants() {
    let wit = MomentWit::default();

    let input = vec![
        Impression::new(
            vec![Stimulus::new(Instant {
                observation: "a dog barked".into(),
            })],
            "Saw a dog",
            None::<String>,
        ),
        Impression::new(
            vec![Stimulus::new(Instant {
                observation: "Pete was startled".into(),
            })],
            "Pete felt startled",
            None::<String>,
        ),
    ];

    let output = wit.digest(&input).await.unwrap();

    assert_eq!(output.stimuli[0].what.summary.contains("dog"), true);
    assert_eq!(output.stimuli[0].what.summary.contains("startled"), true);
}

#[tokio::test]
async fn instant_to_episode_pipeline() {
    let instant_wit = InstantWit::default();
    let moment_wit = MomentWit::default();
    let situation_wit = SituationWit::default();
    let episode_wit = EpisodeWit::default();

    let sensation = Impression::new(
        vec![Stimulus::new(Sensation::HeardUserVoice("hello".into()))],
        "",
        None::<String>,
    );
    let instant = instant_wit.digest(&[sensation]).await.unwrap();
    let moment = moment_wit.digest(&[instant.clone()]).await.unwrap();
    let situation = situation_wit.digest(&[moment.clone()]).await.unwrap();
    let episode = episode_wit.digest(&[situation]).await.unwrap();
    assert!(!episode.stimuli[0].what.summary.is_empty());
}
