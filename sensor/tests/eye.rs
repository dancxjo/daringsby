use sensor::{sensation::{Sensation, SensationData}};

#[test]
fn saw_constructor_sets_fields() {
    let img = vec![1u8, 2, 3];
    let s = Sensation::saw(img.clone());
    assert_eq!(s.how, "eye");
    match s.data {
        Some(SensationData::Image(ref v)) => assert_eq!(v, &img),
        _ => panic!("expected image"),
    }
}
