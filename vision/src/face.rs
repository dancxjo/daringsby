use image::io::Reader as ImageReader;
use rustface::{Detector, ImageData, read_model, create_detector_with_model};
use std::io::Cursor;

fn detector() -> Box<dyn Detector> {
    static MODEL_BYTES: &[u8] = include_bytes!("../model/seeta_fd_frontal_v1.0.bin");
    let model = read_model(Cursor::new(MODEL_BYTES)).expect("load model");
    create_detector_with_model(model)
}

/// Detect faces in a JPEG image and return them as individual JPEG buffers.
pub fn detect_faces(bytes: &[u8]) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut det = detector();
    det.set_min_face_size(40);
    det.set_score_thresh(2.0);
    det.set_pyramid_scale_factor(0.8);
    det.set_slide_window_step(4, 4);

    let img = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode()?;
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let mut image = ImageData::new(&gray, w, h);
    let faces = det.detect(&mut image);
    let mut result = Vec::new();
    for f in faces {
        let bbox = f.bbox();
        let cropped = image::imageops::crop_imm(&img, bbox.x() as u32, bbox.y() as u32, bbox.width(), bbox.height()).to_image();
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut buf).encode_image(&cropped)?;
        result.push(buf);
    }
    Ok(result)
}

/// Embed a face image into a simple fixed-length vector.
pub fn embed_face(bytes: &[u8]) -> Result<Vec<f32>, image::ImageError> {
    use image::imageops::FilterType;
    let img = image::load_from_memory(bytes)?.to_luma8();
    let resized = image::imageops::resize(&img, 32, 32, FilterType::Triangle);
    Ok(resized.into_raw().into_iter().map(|b| b as f32 / 255.0).collect())
}
