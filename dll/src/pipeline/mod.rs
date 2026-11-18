//! Quy trình xử lý một ảnh: nạp → nhận diện → làm thẳng → cắt → xuất (SRS 3.2).
//!
//! M2 chưa nối vào lệnh Tauri hay watcher (D1, D2) nên tạm cho phép dead_code.
#![allow(dead_code)]

pub mod align;
pub mod crop;
pub mod face;
pub mod load;
pub mod preset;

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::ModelStore;

use self::face::Face;

/// Màu đệm khi khung cắt vượt ra ngoài ảnh gốc (D4: nền trắng cho M2).
pub const PADDING_RGB: [u8; 3] = [255, 255, 255];
/// Góc nhỏ hơn ngưỡng này coi như đã thẳng, bỏ qua xoay để tránh mất chi tiết.
pub const STRAIGHTEN_EPSILON_DEG: f32 = 0.05;
/// Mã màu mặc định trong tên tệp ở M2 (chưa có bước tách nền).
pub const DEFAULT_COLOR_TOKEN: &str = "trang";

/// Preset mặc định dùng cho `ProcessOptions::default` (bản đứng lập không có
/// lệnh giao diện nào đặt preset trước khi xử lý).
const DEFAULT_PRESET: &str = "3x4";

/// Tham số xử lý cho một ảnh.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessOptions {
    pub preset_id: String,
    pub straighten_head: bool,
    pub color_token: String,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            preset_id: DEFAULT_PRESET.to_string(),
            straighten_head: true,
            color_token: DEFAULT_COLOR_TOKEN.to_string(),
        }
    }
}

/// Kết quả xử lý một ảnh, dùng cho kiểm thử và cho bước nối giao diện sau này.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessReport {
    pub output_path: PathBuf,
    pub preset_id: &'static str,
    pub output_width: u32,
    pub output_height: u32,
    pub face_count: usize,
    pub face_score: f32,
    pub face_bbox: face::Rect,
    /// Góc nghiêng đo được trước khi làm thẳng, đơn vị độ.
    pub eye_angle_deg: f32,
    /// Góc còn lại sau khi làm thẳng (bằng 0 nếu không xoay).
    pub residual_angle_deg: f32,
    pub straightened: bool,
    /// Tỷ lệ chiều cao mặt trên chiều cao ảnh theo kế hoạch cắt.
    pub face_height_ratio: f32,
}

/// Bộ xử lý giữ sẵn phiên ONNX Runtime để tái sử dụng giữa các ảnh.
pub struct Pipeline {
    detector: face::FaceDetector,
}

impl Pipeline {
    /// Nạp model YuNet đã được xác minh SHA-256 qua `ModelStore`.
    pub fn load(models: &ModelStore) -> AppResult<Self> {
        let model_path = models.yunet_path()?;
        let detector = face::FaceDetector::load(&model_path)?;
        Ok(Self { detector })
    }

    /// Xử lý một ảnh và ghi JPG vào `output_dir`.
    pub fn process(
        &mut self,
        source: &Path,
        output_dir: &Path,
        options: &ProcessOptions,
    ) -> AppResult<ProcessReport> {
        let preset = preset::by_id(&options.preset_id)?;
        let image = load::load_normalized(source)?;
        let faces = self.detector.detect(&image)?;
        let face_count = faces.len();
        let detected = select_single_face(faces)?;
        let face_score = detected.score;

        let measured_angle = align::eye_angle_deg(&detected);
        let straightened = options.straighten_head && measured_angle.abs() > STRAIGHTEN_EPSILON_DEG;
        let (working_image, working_face) = if straightened {
            align::straighten(&image, &detected, measured_angle, PADDING_RGB)
        } else {
            (image, detected)
        };

        let plan = crop::plan(&working_face, preset)?;
        let face_height_ratio = crop::face_height_ratio(&working_face, &plan);
        let output = crop::render(&working_image, &plan, preset, PADDING_RGB);

        let file_name = export::output_file_name(source, preset.id, &options.color_token);
        let output_path = export::unique_output_path(output_dir, &file_name)?;
        export::write_jpeg(&output, &output_path)?;

        Ok(ProcessReport {
            output_path,
            preset_id: preset.id,
            output_width: preset.width,
            output_height: preset.height,
            face_count,
            face_score,
            face_bbox: working_face.bbox,
            eye_angle_deg: measured_angle,
            residual_angle_deg: align::eye_angle_deg(&working_face),
            straightened,
            face_height_ratio,
        })
    }
}

/// Xử lý một ảnh với model lấy từ `models`. Dùng cho kiểm thử và cho lệnh một ảnh.
pub fn process_single_image(
    source: &Path,
    output_dir: &Path,
    models: &ModelStore,
    options: &ProcessOptions,
) -> AppResult<ProcessReport> {
    let mut pipeline = Pipeline::load(models)?;
    pipeline.process(source, output_dir, options)
}

/// Nạp pipeline từ model nhúng trong DLL (single-file): xác minh SHA-256 với
/// manifest nhúng rồi mở session từ bộ nhớ.
pub fn pipeline_embedded() -> AppResult<Pipeline> {
    crate::embed::verify_embedded()?;
    let detector = face::FaceDetector::load_from_bytes(crate::embed::YUNET_ONNX)?;
    Ok(Pipeline { detector })
}

/// Xử lý một ảnh bằng model nhúng (single-file). Fallback khi không có thư mục
/// model hợp lệ nào.
pub fn process_single_image_embedded(
    source: &Path,
    output_dir: &Path,
    options: &ProcessOptions,
) -> AppResult<ProcessReport> {
    let mut pipeline = pipeline_embedded()?;
    pipeline.process(source, output_dir, options)
}

/// Yêu cầu đúng một khuôn mặt: không có → E002, nhiều hơn một → E003.
pub fn select_single_face(mut faces: Vec<Face>) -> AppResult<Face> {
    if faces.is_empty() {
        return Err(AppError::face_not_found());
    }
    if faces.len() > 1 {
        return Err(AppError::multiple_faces());
    }
    faces.pop().ok_or_else(AppError::face_not_found)
}
