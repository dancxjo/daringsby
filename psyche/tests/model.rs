use chrono::{DateTime, Local, Utc};
use psyche::model::{Experience, Impression, Stimulus};

#[test]
fn impression_can_be_nested() {
    let stim1 = Stimulus::new("sound");
    let imp1 = Impression::new(vec![stim1.clone()], "heard", Some("👂"));
    let stim2 = Stimulus::new(imp1.clone());
    let imp2 = Impression::new(vec![stim2], "meta", None::<String>);
    assert_eq!(imp2.stimuli.len(), 1);
    if let Some(first) = imp2.stimuli.first() {
        let inner = &first.what;
        assert_eq!(inner.summary, "heard");
    }
}

#[test]
fn experience_wraps_impression() {
    let stim = Stimulus::new("hi");
    let imp = Impression::new(vec![stim], "greeting", None::<String>);
    let exp = Experience::new(imp.clone(), vec![0.1]);
    assert_eq!(exp.impression.summary, "greeting");
    assert_eq!(exp.embedding.len(), 1);
}

#[test]
fn impression_collects_source_sensation_ids_from_stimuli() {
    let stim = Stimulus::with_source_sensation_ids(
        "hi",
        Utc::now(),
        ["sensation:utterance:1", "sensation:utterance:1"],
    );
    let imp = Impression::new(vec![stim], "greeting", None::<String>);

    assert_eq!(imp.source_sensation_ids, vec!["sensation:utterance:1"]);
}

#[test]
fn prompt_list_items_include_localized_timestamps() {
    let timestamp = DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&Utc);
    let expected = timestamp
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string();
    let stim = Stimulus {
        what: "hi",
        timestamp,
        source_sensation_ids: Vec::new(),
    };
    let imp = Impression {
        stimuli: vec![stim.clone()],
        source_sensation_ids: Vec::new(),
        summary: "greeting".into(),
        emoji: None,
        timestamp,
    };
    let exp = Experience::new(imp.clone(), vec![0.1]);

    assert_eq!(stim.prompt_list_item(), format!("[{expected}] hi"));
    assert_eq!(imp.prompt_list_item(), format!("[{expected}] greeting"));
    assert!(
        exp.prompt_list_item()
            .starts_with(&format!("[{expected}] greeting"))
    );
}
