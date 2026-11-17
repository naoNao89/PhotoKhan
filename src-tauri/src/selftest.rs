//! Tự kiểm tra (self-test) đường dẫn sản xuất trước khi khởi động Tauri.
//!
//! Chạy headless, không cần context Tauri: `photokhan --self-test` xác minh
//! model YuNet (manifest + kích thước + SHA-256), nạp model bằng ONNX Runtime,
//! phát hiện khuôn mặt trên một ảnh tổng hợp (không RNG, không đọc file ảnh)
//! và in MỘT tài liệu JSON duy nhất ra stdout.
//!
//! Thư mục model được chọn theo thứ tự ưu tiên — mục đầu tiên có `manifest.json`
//! sẽ thắng:
//!   1. Biến môi trường `PHOTOKHAN_MODELS_DIR` (dùng khi chạy bản dev/CI).
//!   2. Thư mục chứa file thực thi (trên Windows, `resource_dir()` của Tauri
//!      trỏ tới thư mục cạnh file `.exe`).
//!   3. Thư mục `resources` cạnh file thực thi.
//!   4. Thư mục `models` cạnh file thực thi (quy ước beside_executable).
//!
//! Mã thoát: 0 = thành công, 1 = lỗi (mọi lỗi đều được ghi vào JSON `error`).

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::models::{MANIFEST_FILE_NAME, ModelStore, YUNET_MODEL_ID};
use crate::pipeline::face::{Face, FaceDetector, MODEL_INPUT_SIZE};

/// Biến môi trường ghi đè thư mục model (dùng trong CI khi chạy bản dev).
const MODELS_DIR_ENV: &str = "PHOTOKHAN_MODELS_DIR";

/// Một thư mục ứng viên trong quá trình giải quyết thư mục model.
struct Candidate {
    label: &'static str,
    path: PathBuf,
    manifest_found: bool,
}

/// Kết quả giải quyết thư mục model: danh sách ứng viên và mục được chọn.
struct Resolution {
    env_override: bool,
    candidates: Vec<Candidate>,
    selected: Option<usize>,
}

impl Resolution {
    /// Chuyển thành phần `resolution` của báo cáo JSON.
    fn to_json(&self) -> serde_json::Value {
        let candidates: Vec<serde_json::Value> = self
            .candidates
            .iter()
            .map(|candidate| {
                serde_json::json!({
                    "label": candidate.label,
                    "path": candidate.path,
                    "manifest_found": candidate.manifest_found,
                })
            })
            .collect();
        let selected = self.selected.map(|index| self.candidates[index].label);
        serde_json::json!({
            "env_override": self.env_override,
            "candidates": candidates,
            "selected": selected,
        })
    }
}

/// Chạy self-test và trả về mã thoát cho tiến trình (`--self-test`).
pub fn run() -> i32 {
    let resolution = resolve();
    let (report, success) = match resolution.selected {
        Some(index) => {
            let dir = resolution.candidates[index].path.clone();
            match verify_and_detect(&dir) {
                Ok(model) => (
                    serde_json::json!({
                        "status": "ok",
                        "resolution": resolution.to_json(),
                        "model": model,
                        "error": serde_json::Value::Null,
                    }),
                    true,
                ),
                Err(error) => (error_report(&resolution, error), false),
            }
        }
        None => (error_report(&resolution, AppError::model_invalid()), false),
    };

    println!("{report}");
    if success { 0 } else { 1 }
}

/// Giải quyết thư mục model theo thứ tự ưu tiên; ghi lại mọi ứng viên.
fn resolve() -> Resolution {
    let env_override = std::env::var_os(MODELS_DIR_ENV).is_some();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));

    let mut candidates: Vec<Candidate> = Vec::new();
    if let Some(dir) = std::env::var_os(MODELS_DIR_ENV) {
        candidates.push(Candidate {
            label: "PHOTOKHAN_MODELS_DIR",
            path: PathBuf::from(dir),
            manifest_found: false,
        });
    }
    if let Some(dir) = &exe_dir {
        candidates.push(Candidate {
            label: "exe_dir",
            path: dir.clone(),
            manifest_found: false,
        });
        candidates.push(Candidate {
            label: "exe_dir/resources",
            path: dir.join("resources"),
            manifest_found: false,
        });
        candidates.push(Candidate {
            label: "exe_dir/models",
            path: dir.join("models"),
            manifest_found: false,
        });
    }

    let mut selected = None;
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.manifest_found = candidate.path.join(MANIFEST_FILE_NAME).is_file();
        if selected.is_none() && candidate.manifest_found {
            selected = Some(index);
        }
    }

    Resolution {
        env_override,
        candidates,
        selected,
    }
}

/// Chạy toàn bộ pipeline sản xuất trên thư mục model đã chọn.
fn verify_and_detect(models_dir: &Path) -> AppResult<serde_json::Value> {
    let model_path = ModelStore::new(models_dir).yunet_path()?;

    let actual_sha256 = sha256_file(&model_path)?;
    let expected_sha256 = expected_sha256_from_manifest(models_dir)?;
    let sha256_match = actual_sha256.eq_ignore_ascii_case(expected_sha256.trim());
    let size_bytes = fs::metadata(&model_path)
        .map_err(|_| AppError::model_invalid())?
        .len();

    let mut detector = FaceDetector::load(&model_path)?;
    let image = synthetic_image(MODEL_INPUT_SIZE, MODEL_INPUT_SIZE);
    let faces = detector.detect(&image)?;
    validate_faces(&faces, &image)?;

    let max_score = faces.iter().map(|face| face.score).reduce(f32::max);

    Ok(serde_json::json!({
        "path": model_path,
        "sha256": actual_sha256,
        "expected_sha256": expected_sha256,
        "sha256_match": sha256_match,
        "size_bytes": size_bytes,
        "input_size": MODEL_INPUT_SIZE,
        "faces": faces.len(),
        "max_score": max_score,
    }))
}

/// Ảnh tổng hợp với gradient xác định: giá trị kênh = (x * 7 + y * 13) % 256.
fn synthetic_image(width: u32, height: u32) -> ::image::RgbImage {
    let mut image = ::image::RgbImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let value = ((x * 7 + y * 13) % 256) as u8;
        *pixel = ::image::Rgb([value; 3]);
    }
    image
}

/// Kiểm tra đầu ra của detector. Không có khuôn mặt nào là kết quả hợp lệ
/// (ảnh nhiễu); mọi vi phạm về điểm số/toạ độ đều là lỗi.
fn validate_faces(faces: &[Face], image: &::image::RgbImage) -> AppResult<()> {
    let width = image.width() as f32;
    let height = image.height() as f32;
    for face in faces {
        let rect = &face.bbox;
        let valid = face.score.is_finite()
            && (0.0..=1.0).contains(&face.score)
            && rect.x.is_finite()
            && rect.y.is_finite()
            && rect.width.is_finite()
            && rect.height.is_finite()
            && rect.width > 0.0
            && rect.height > 0.0
            && rect.x + rect.width <= width + 0.001
            && rect.y + rect.height <= height + 0.001;
        if !valid {
            return Err(face_output_invalid());
        }
    }
    Ok(())
}

/// Đọc `manifest.json` và lấy giá trị `sha256` của mục có `id == "yunet"`.
fn expected_sha256_from_manifest(models_dir: &Path) -> AppResult<String> {
    let manifest_path = models_dir.join(MANIFEST_FILE_NAME);
    let bytes = fs::read(&manifest_path).map_err(|_| AppError::model_invalid())?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| AppError::model_invalid())?;
    let sha256 = manifest
        .get("models")
        .and_then(serde_json::Value::as_array)
        .and_then(|models| {
            models.iter().find(|entry| {
                entry.get("id").and_then(serde_json::Value::as_str) == Some(YUNET_MODEL_ID)
            })
        })
        .and_then(|entry| entry.get("sha256"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(AppError::model_invalid)?;
    Ok(sha256.to_string())
}

/// Tính SHA-256 của file model (hex chữ thường).
fn sha256_file(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path).map_err(|_| AppError::model_invalid())?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

/// Báo cáo lỗi theo cấu trúc thống nhất với CONTRACT.md.
fn error_report(resolution: &Resolution, error: AppError) -> serde_json::Value {
    serde_json::json!({
        "status": "error",
        "resolution": resolution.to_json(),
        "model": serde_json::Value::Null,
        "error": error,
    })
}

/// Lỗi khi detector trả về toạ độ hoặc điểm số ngoài phạm vi cho phép.
fn face_output_invalid() -> AppError {
    AppError::new(
        "E004",
        "Kết quả phát hiện khuôn mặt không hợp lệ (toạ độ hoặc điểm số vượt phạm vi).",
        "Hãy sao chép lại thư mục models cạnh ứng dụng rồi thử lại.",
    )
}
