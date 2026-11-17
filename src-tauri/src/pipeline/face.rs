use std::path::Path;
use std::sync::OnceLock;

use ::image::RgbImage;
use ::image::imageops::FilterType;
use ort::environment::Environment;
use ort::session::{Session, SessionOutputs};
use ort::value::Tensor;

use crate::error::{AppError, AppResult};

/// Kích thước đầu vào cố định của `face_detection_yunet_2023mar.onnx`
/// (đọc trực tiếp từ shape của model: 1x3x640x640).
pub const MODEL_INPUT_SIZE: u32 = 640;
pub const SCORE_THRESHOLD: f32 = 0.6;
pub const NMS_IOU_THRESHOLD: f32 = 0.3;
pub const TOP_K: usize = 5000;

const STRIDES: [u32; 3] = [8, 16, 32];

static ORT_ENVIRONMENT: OnceLock<Environment> = OnceLock::new();

/// Khởi tạo ONNX Runtime Environment tường minh, đúng một lần cho cả tiến trình.
fn environment() -> AppResult<&'static Environment> {
    if let Some(environment) = ORT_ENVIRONMENT.get() {
        return Ok(environment);
    }

    let built = ort::init()
        .with_name("photokhan")
        .with_telemetry(false)
        .build()
        .map_err(|_| AppError::model_runtime())?;

    Ok(ORT_ENVIRONMENT.get_or_init(|| built))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

/// Kết quả phát hiện một khuôn mặt.
///
/// Thứ tự landmark theo YuNet: mắt phải, mắt trái, đầu mũi, khoé miệng phải,
/// khoé miệng trái (theo góc nhìn của người trong ảnh).
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub bbox: Rect,
    pub landmarks: [Point; 5],
    pub score: f32,
}

impl Face {
    pub fn right_eye(&self) -> Point {
        self.landmarks[0]
    }

    pub fn left_eye(&self) -> Point {
        self.landmarks[1]
    }
}

pub struct FaceDetector {
    session: Session,
    input_name: String,
    /// Shape (dims) của 12 output YuNet từ lần `detect()` gần nhất (tên, dims).
    output_shapes: Vec<(String, Vec<i64>)>,
}

impl FaceDetector {
    /// Nạp YuNet. Model phải đã được xác minh SHA-256 trước đó.
    pub fn load(model_path: &Path) -> AppResult<Self> {
        let environment = environment()?;

        let session = Session::builder(environment)
            .map_err(|_| AppError::model_runtime())?
            .with_intra_threads(1)
            .map_err(|_| AppError::model_runtime())?
            .commit_from_file(model_path)
            .map_err(|_| AppError::model_runtime())?;

        let inputs = session.inputs();
        if inputs.len() != 1 {
            return Err(AppError::model_runtime());
        }
        let input_name = inputs[0].name().to_string();

        Ok(Self {
            session,
            input_name,
            output_shapes: Vec::new(),
        })
    }

    /// Shape (dims) của các output YuNet từ lần `detect()` gần nhất.
    pub fn output_shapes(&self) -> &[(String, Vec<i64>)] {
        &self.output_shapes
    }

    /// Phát hiện khuôn mặt trên ảnh RGB 8-bit. Trả về toạ độ theo pixel ảnh gốc.
    pub fn detect(&mut self, image: &RgbImage) -> AppResult<Vec<Face>> {
        if image.width() == 0 || image.height() == 0 {
            return Ok(Vec::new());
        }

        let resized = ::image::imageops::resize(
            image,
            MODEL_INPUT_SIZE,
            MODEL_INPUT_SIZE,
            FilterType::Triangle,
        );

        // YuNet nhận BGR, giá trị 0..255, không trừ mean (tham chiếu OpenCV).
        let plane = (MODEL_INPUT_SIZE * MODEL_INPUT_SIZE) as usize;
        let mut data = vec![0_f32; plane * 3];
        for (index, pixel) in resized.as_raw().chunks_exact(3).enumerate() {
            data[index] = f32::from(pixel[2]);
            data[plane + index] = f32::from(pixel[1]);
            data[2 * plane + index] = f32::from(pixel[0]);
        }

        let tensor = Tensor::from_array((
            [
                1_usize,
                3,
                MODEL_INPUT_SIZE as usize,
                MODEL_INPUT_SIZE as usize,
            ],
            data,
        ))
        .map_err(|_| AppError::model_runtime())?;

        let input_name = self.input_name.clone();
        let outputs = self
            .session
            .run(ort::inputs![input_name => tensor])
            .map_err(|_| AppError::model_runtime())?;

        let scale_x = image.width() as f32 / MODEL_INPUT_SIZE as f32;
        let scale_y = image.height() as f32 / MODEL_INPUT_SIZE as f32;
        let mut candidates = Vec::new();
        let mut output_shapes = Vec::new();

        for stride in STRIDES {
            let cells = ((MODEL_INPUT_SIZE / stride) * (MODEL_INPUT_SIZE / stride)) as usize;
            let (cls_dims, cls) = output_tensor(&outputs, &format!("cls_{stride}"))?;
            let (obj_dims, obj) = output_tensor(&outputs, &format!("obj_{stride}"))?;
            let (bbox_dims, bbox) = output_tensor(&outputs, &format!("bbox_{stride}"))?;
            let (kps_dims, kps) = output_tensor(&outputs, &format!("kps_{stride}"))?;

            // Số phần tử phải khớp chính xác (không chỉ >=): shape sai hoặc NaN
            // trong tensor sẽ bị từ chối thay vì âm thầm cho 0 khuôn mặt.
            validate_tensor(cls, cells)?;
            validate_tensor(obj, cells)?;
            validate_tensor(bbox, cells * 4)?;
            validate_tensor(kps, cells * 10)?;

            output_shapes.push((format!("cls_{stride}"), cls_dims));
            output_shapes.push((format!("obj_{stride}"), obj_dims));
            output_shapes.push((format!("bbox_{stride}"), bbox_dims));
            output_shapes.push((format!("kps_{stride}"), kps_dims));

            decode_level(
                cls,
                obj,
                bbox,
                kps,
                stride,
                scale_x,
                scale_y,
                &mut candidates,
            );
        }
        self.output_shapes = output_shapes;

        Ok(non_maximum_suppression(candidates))
    }
}

fn output_tensor<'outputs>(
    outputs: &'outputs SessionOutputs<'_>,
    name: &str,
) -> AppResult<(Vec<i64>, &'outputs [f32])> {
    let value = outputs.get(name).ok_or_else(AppError::model_runtime)?;
    let (view, data) = value
        .try_extract_tensor::<f32>()
        .map_err(|_| AppError::model_runtime())?;
    Ok((view.shape().to_vec(), data))
}

/// Kiểm tra một tensor output: số phần tử khớp chính xác và mọi giá trị hữu hạn.
fn validate_tensor(data: &[f32], expected: usize) -> AppResult<()> {
    if data.len() != expected {
        return Err(AppError::model_runtime());
    }
    if !data.iter().all(|value| value.is_finite()) {
        return Err(AppError::model_runtime());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decode_level(
    cls: &[f32],
    obj: &[f32],
    bbox: &[f32],
    kps: &[f32],
    stride: u32,
    scale_x: f32,
    scale_y: f32,
    faces: &mut Vec<Face>,
) {
    let cols = (MODEL_INPUT_SIZE / stride) as usize;
    let stride_f = stride as f32;

    for row in 0..cols {
        for col in 0..cols {
            let index = row * cols + col;
            let cls_score = cls[index].clamp(0.0, 1.0);
            let obj_score = obj[index].clamp(0.0, 1.0);
            let score = (cls_score * obj_score).sqrt();
            if !score.is_finite() || score < SCORE_THRESHOLD {
                continue;
            }

            let center_x = (col as f32 + bbox[index * 4]) * stride_f;
            let center_y = (row as f32 + bbox[index * 4 + 1]) * stride_f;
            let width = bbox[index * 4 + 2].exp() * stride_f;
            let height = bbox[index * 4 + 3].exp() * stride_f;

            if !width.is_finite() || !height.is_finite() || width <= 1.0 || height <= 1.0 {
                continue;
            }

            let landmarks = std::array::from_fn(|point| Point {
                x: (kps[index * 10 + point * 2] + col as f32) * stride_f * scale_x,
                y: (kps[index * 10 + point * 2 + 1] + row as f32) * stride_f * scale_y,
            });

            faces.push(Face {
                bbox: Rect {
                    x: (center_x - width / 2.0) * scale_x,
                    y: (center_y - height / 2.0) * scale_y,
                    width: width * scale_x,
                    height: height * scale_y,
                },
                landmarks,
                score,
            });
        }
    }
}

fn non_maximum_suppression(mut faces: Vec<Face>) -> Vec<Face> {
    faces.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    faces.truncate(TOP_K);

    let mut kept: Vec<Face> = Vec::new();
    for face in faces {
        let overlaps = kept
            .iter()
            .any(|other| intersection_over_union(&other.bbox, &face.bbox) > NMS_IOU_THRESHOLD);
        if !overlaps {
            kept.push(face);
        }
    }
    kept
}

fn intersection_over_union(left: &Rect, right: &Rect) -> f32 {
    let overlap_width = left.right().min(right.right()) - left.x.max(right.x);
    let overlap_height = left.bottom().min(right.bottom()) - left.y.max(right.y);
    if overlap_width <= 0.0 || overlap_height <= 0.0 {
        return 0.0;
    }

    let intersection = overlap_width * overlap_height;
    let union = left.width * left.height + right.width * right.height - intersection;
    if union <= 0.0 {
        return 0.0;
    }
    intersection / union
}

#[cfg(test)]
mod tests {
    use super::{
        Face, Point, Rect, intersection_over_union, non_maximum_suppression, validate_tensor,
    };

    fn face(x: f32, y: f32, score: f32) -> Face {
        Face {
            bbox: Rect {
                x,
                y,
                width: 10.0,
                height: 10.0,
            },
            landmarks: [Point { x, y }; 5],
            score,
        }
    }

    #[test]
    fn iou_of_identical_boxes_is_one() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        assert!((intersection_over_union(&rect, &rect) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn nms_keeps_the_highest_score_of_overlapping_boxes() {
        let kept = non_maximum_suppression(vec![face(0.0, 0.0, 0.9), face(1.0, 1.0, 0.8)]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].score, 0.9);
    }

    #[test]
    fn nms_keeps_separate_boxes() {
        let kept = non_maximum_suppression(vec![face(0.0, 0.0, 0.9), face(50.0, 50.0, 0.8)]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn tensor_with_wrong_length_is_rejected() {
        assert!(validate_tensor(&[0.0; 4], 5).is_err());
        assert!(validate_tensor(&[0.0; 5], 5).is_ok());
    }

    #[test]
    fn tensor_with_non_finite_values_is_rejected() {
        assert!(validate_tensor(&[0.0, f32::NAN, 0.0], 3).is_err());
        assert!(validate_tensor(&[0.0, f32::INFINITY, 0.0], 3).is_err());
        assert!(validate_tensor(&[0.0, 0.5, 1.0], 3).is_ok());
    }
}
