use pete::EventBus;

#[test]
fn custom_capacities() {
    let custom = 8;
    let (bus, _rx) = EventBus::with_capacities(custom, custom * 2, custom);
    bus.publish_event(psyche::Event::EmotionChanged("🙂".into()));
    bus.log("ok");
    // Custom constructor should operate like the default without panicking
    let _ = bus.subscribe_events();
    let _ = bus.subscribe_logs();
}
