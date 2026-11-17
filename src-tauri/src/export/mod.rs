//! Ghi ảnh JPG đầu ra theo CONTRACT/SRS 9.3. Chưa nối lệnh Tauri ở M2 (D1).
#![allow(dead_code)]

pub mod image;

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

pub use image::write_jpeg;

/// Tên file xuất theo mẫu mặc định ở SRS 9.3: `{tên_gốc}_{preset}_{màu}.jpg`.
pub fn output_file_name(original: &Path, preset_id: &str, color_token: &str) -> String {
    let stem = original
        .file_stem()
        .map(|value| value.to_string_lossy())
        .map(|value| sanitize_component(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "anh".to_string());

    let preset = sanitize_component(preset_id);
    let color = sanitize_component(color_token);
    let preset = if preset.is_empty() {
        "preset".to_string()
    } else {
        preset
    };
    let color = if color.is_empty() {
        "mau".to_string()
    } else {
        color
    };

    format!("{stem}_{preset}_{color}.jpg")
}

/// Chọn đường dẫn chưa tồn tại; nếu trùng thì thêm `_2`, `_3`, ...
pub fn unique_output_path(dir: &Path, file_name: &str) -> AppResult<PathBuf> {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return Ok(candidate);
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .ok_or_else(AppError::output_write_failed)?;
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "jpg".to_string());

    for index in 2..=9999_u32 {
        let candidate = dir.join(format!("{stem}_{index}.{extension}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::output_write_failed())
}

fn sanitize_component(value: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    let mut last_was_separator = false;

    for character in value.chars() {
        if character.is_alphanumeric() || character == '-' || character == '_' {
            cleaned.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            cleaned.push('_');
            last_was_separator = true;
        }
    }

    cleaned
        .trim_matches(|character| character == '_' || character == '.' || character == ' ')
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{output_file_name, unique_output_path};

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
    fn file_name_matches_section_9_3() {
        assert_eq!(
            output_file_name(
                PathBuf::from(r"C:\anh\Nguyen Van A.JPG").as_path(),
                "3x4",
                "trang"
            ),
            "Nguyen_Van_A_3x4_trang.jpg"
        );
        assert_eq!(
            output_file_name(
                PathBuf::from("img 1 normal.jpg").as_path(),
                "4x6",
                "xanhduong"
            ),
            "img_1_normal_4x6_xanhduong.jpg"
        );
    }

    #[test]
    fn file_name_removes_windows_forbidden_characters() {
        let name = output_file_name(
            PathBuf::from(r#"anh:<>"/\|?*.png"#).as_path(),
            "3x4",
            "trang",
        );
        assert_eq!(name, "anh_3x4_trang.jpg");
    }

    #[test]
    fn duplicate_names_get_a_suffix() {
        let dir = unique_dir("unique-name");
        let first = unique_output_path(&dir, "anh_3x4_trang.jpg").unwrap();
        fs::write(&first, b"x").unwrap();
        let second = unique_output_path(&dir, "anh_3x4_trang.jpg").unwrap();

        assert_eq!(first.file_name().unwrap(), "anh_3x4_trang.jpg");
        assert_eq!(second.file_name().unwrap(), "anh_3x4_trang_2.jpg");
    }
}
