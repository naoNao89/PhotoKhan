//! Model + manifest nhúng trực tiếp vào DLL (single-file mode).
//!
//! `phk_process` dùng model nhúng khi không tìm thấy thư mục model nào hợp lệ
//! (override → env → `models\` cạnh DLL → `models\` cạnh host exe). SHA-256 vẫn
//! được xác minh trước khi nạp: so khớp hash của model nhúng với `sha256` trong
//! manifest nhúng (E004 nếu lệch).

use crate::error::{AppError, AppResult};

pub const MANIFEST_JSON: &[u8] = include_bytes!("../../models/manifest.json");
pub const YUNET_ONNX: &[u8] = include_bytes!("../../models/face_detection_yunet_2023mar.onnx");

/// Xác minh SHA-256 của model nhúng với manifest nhúng.
pub fn verify_embedded() -> AppResult<()> {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(YUNET_ONNX);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }

    let manifest: serde_json::Value =
        serde_json::from_slice(MANIFEST_JSON).map_err(|_| AppError::model_invalid())?;
    let expected = manifest
        .get("models")
        .and_then(serde_json::Value::as_array)
        .and_then(|models| {
            models.iter().find(|entry| {
                entry.get("id").and_then(serde_json::Value::as_str)
                    == Some(crate::models::YUNET_MODEL_ID)
            })
        })
        .and_then(|entry| entry.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(AppError::model_invalid)?;

    if !hex.eq_ignore_ascii_case(expected.trim()) {
        return Err(AppError::model_invalid());
    }
    Ok(())
}
