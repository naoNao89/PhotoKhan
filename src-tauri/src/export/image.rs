use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use ::image::RgbImage;
use ::image::codecs::jpeg::{JpegEncoder, PixelDensity};

use crate::error::{AppError, AppResult};

pub const JPEG_QUALITY: u8 = 95;
pub const JPEG_DPI: u16 = 300;

/// Ghi JPG chất lượng 95, DPI 300, dùng file tạm rồi đổi tên để tránh file dở.
pub fn write_jpeg(image: &RgbImage, path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| AppError::output_write_failed())?;
    }

    let temp_path = path.with_extension("jpg.part");
    let result = write_temp(image, &temp_path);

    match result {
        Ok(()) => {
            if fs::rename(&temp_path, path).is_err() {
                let _ = fs::remove_file(&temp_path);
                return Err(AppError::output_write_failed());
            }
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

fn write_temp(image: &RgbImage, temp_path: &Path) -> AppResult<()> {
    let file = File::create(temp_path).map_err(|_| AppError::output_write_failed())?;
    let mut writer = BufWriter::new(file);

    {
        let mut encoder = JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY);
        encoder.set_pixel_density(PixelDensity::dpi(JPEG_DPI));
        encoder
            .encode_image(image)
            .map_err(|_| AppError::output_write_failed())?;
    }

    writer
        .flush()
        .map_err(|_| AppError::output_write_failed())?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|_| AppError::output_write_failed())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use ::image::{Rgb, RgbImage};

    use super::{JPEG_DPI, JPEG_QUALITY, write_jpeg};

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
    fn writes_jfif_with_dpi_300_and_no_temp_leftover() {
        let dir = unique_dir("jpeg-dpi");
        let path = dir.join("anh.jpg");
        let image = RgbImage::from_pixel(10, 12, Rgb([1, 2, 3]));

        write_jpeg(&image, &path).unwrap();

        let bytes = fs::read(&path).unwrap();
        let position = bytes
            .windows(5)
            .position(|window| window == b"JFIF\0")
            .expect("JFIF header");
        assert_eq!(bytes[position + 7], 1);
        assert_eq!(
            u16::from_be_bytes([bytes[position + 8], bytes[position + 9]]),
            JPEG_DPI
        );
        assert_eq!(
            u16::from_be_bytes([bytes[position + 10], bytes[position + 11]]),
            JPEG_DPI
        );
        assert_eq!(JPEG_QUALITY, 95);
        assert!(!dir.join("anh.jpg.part").exists());
    }
}
