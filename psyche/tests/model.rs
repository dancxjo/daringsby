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
