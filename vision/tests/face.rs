use vision::face::{detect_faces, embed_face};

#[test]
fn embed_face_size() {
    let img = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]));
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut buf).encode_image(&img).unwrap();
    let vec = embed_face(&buf).unwrap();
    assert_eq!(vec.len(), 32 * 32);
}

#[test]
fn detect_faces_none() {
    // a blank image should yield no faces
    let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]));
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut buf).encode_image(&img).unwrap();
    let faces = detect_faces(&buf).unwrap();
    assert!(faces.is_empty());
}
