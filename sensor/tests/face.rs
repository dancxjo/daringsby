use sensor::{sensation::{Sensation, SensationData}};

#[test]
fn saw_face_sets_fields() {
    let img = vec![4u8, 5, 6];
    let s = Sensation::saw_face(img.clone());
    assert_eq!(s.how, "face");
    match s.data {
        Some(SensationData::Image(ref v)) => assert_eq!(v, &img),
        _ => panic!("expected image"),
    }
}
