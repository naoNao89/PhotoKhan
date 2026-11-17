use std::path::Path;

use ::image::imageops::FilterType;
use ::image::metadata::Orientation;
use ::image::{DynamicImage, ImageDecoder, ImageReader, RgbImage};

use crate::error::{AppError, AppResult};

/// Giới hạn cạnh dài để tiết kiệm RAM (SRS 3.2 bước 1).
pub const MAX_LONG_EDGE: u32 = 6000;
/// Ảnh nhỏ hơn ngưỡng này được coi là không hợp lệ (SRS phụ lục B, E001).
pub const MIN_EDGE: u32 = 64;

const UNSUPPORTED_EXTENSIONS: [&str; 2] = ["heic", "heif"];

/// Đọc ảnh, áp EXIF orientation, chuyển RGB 8-bit và giới hạn cạnh dài 6000 px.
pub fn load_normalized(path: &Path) -> AppResult<RgbImage> {
    if has_unsupported_extension(path) {
        return Err(AppError::unsupported_format());
    }

    let reader = ImageReader::open(path).map_err(|_| AppError::image_unreadable())?;
    let reader = reader
        .with_guessed_format()
        .map_err(|_| AppError::image_unreadable())?;
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| AppError::image_unreadable())?;
    let orientation = decoder
        .orientation()
        .map_err(|_| AppError::image_unreadable())?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|_| AppError::image_unreadable())?;

    if image.width() < MIN_EDGE || image.height() < MIN_EDGE {
        return Err(AppError::image_unreadable());
    }

    if orientation != Orientation::NoTransforms {
        image.apply_orientation(orientation);
    }

    let long_edge = image.width().max(image.height());
    if long_edge > MAX_LONG_EDGE {
        let scale = MAX_LONG_EDGE as f32 / long_edge as f32;
        let width = ((image.width() as f32 * scale).round() as u32).max(1);
        let height = ((image.height() as f32 * scale).round() as u32).max(1);
        image = image.resize(width, height, FilterType::Lanczos3);
    }

    Ok(image.to_rgb8())
}

fn has_unsupported_extension(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    UNSUPPORTED_EXTENSIONS.contains(&extension.as_str())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ::image::codecs::jpeg::JpegEncoder;
    use ::image::{Rgb, RgbImage};

    use super::{MAX_LONG_EDGE, MIN_EDGE, load_normalized};

    fn unique_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("đồng hồ hệ thống")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "photokhan-test-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("tạo thư mục tạm");
        path
    }

    #[test]
    fn corrupt_file_returns_e001() {
        let dir = unique_dir("load-corrupt");
        let path = dir.join("hong.jpg");
        fs::write(&path, b"khong phai anh").unwrap();

        let error = load_normalized(&path).unwrap_err();
        assert_eq!(error.code, "E001");
    }

    #[test]
    fn missing_file_returns_e001() {
        let dir = unique_dir("load-missing");
        let error = load_normalized(&dir.join("khong-ton-tai.jpg")).unwrap_err();
        assert_eq!(error.code, "E001");
    }

    #[test]
    fn heic_extension_returns_e001_with_unsupported_message() {
        let dir = unique_dir("load-heic");
        let path = dir.join("anh.heic");
        fs::write(&path, b"bat ky").unwrap();

        let error = load_normalized(&path).unwrap_err();
        assert_eq!(error.code, "E001");
        assert!(error.message_vi.contains("chưa được hỗ trợ"));
    }

    #[test]
    fn exif_orientation_six_rotates_the_image() {
        let dir = unique_dir("load-exif");
        let path = dir.join("xoay.jpg");
        let image = RgbImage::from_pixel(200, 80, Rgb([200, 30, 40]));
        fs::write(&path, jpeg_with_orientation(&image, 6)).unwrap();

        let loaded = load_normalized(&path).unwrap();
        assert_eq!(loaded.dimensions(), (80, 200));
    }

    #[test]
    fn long_edge_is_clamped_to_6000_pixels() {
        let dir = unique_dir("load-clamp");
        let path = dir.join("dai.png");
        let image = RgbImage::from_pixel(6500, 100, Rgb([10, 200, 30]));
        image.save(&path).unwrap();

        let loaded = load_normalized(&path).unwrap();
        assert_eq!(loaded.width(), MAX_LONG_EDGE);
        assert!(loaded.height() < 100);
        assert!(loaded.height() >= MIN_EDGE);
    }

    /// Ghép một APP1 Exif tối giản (chỉ có tag Orientation) vào JPG đã mã hoá.
    fn jpeg_with_orientation(image: &RgbImage, orientation: u16) -> Vec<u8> {
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(image)
            .unwrap();

        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42_u16.to_le_bytes());
        tiff.extend_from_slice(&8_u32.to_le_bytes());
        tiff.extend_from_slice(&1_u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
        tiff.extend_from_slice(&3_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&u32::from(orientation).to_le_bytes());
        tiff.extend_from_slice(&0_u32.to_le_bytes());

        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&tiff);

        let mut segment = vec![0xFF, 0xE1];
        segment.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(&payload);

        let mut output = jpeg[..2].to_vec();
        output.extend_from_slice(&segment);
        output.extend_from_slice(&jpeg[2..]);
        output
    }
}
