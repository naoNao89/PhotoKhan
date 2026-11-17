use ::image::{Rgb, RgbImage};

use crate::error::{AppError, AppResult};

use super::align::{eye_center, sample_bilinear};
use super::face::Face;
use super::preset::Preset;

/// Khung cắt trong toạ độ ảnh nguồn. `left`/`top` là vị trí pixel (0,0) của ảnh
/// đầu ra trong ảnh nguồn; `scale` là tỷ lệ phóng giữa nguồn và đích.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropPlan {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

pub fn plan(face: &Face, preset: Preset) -> AppResult<CropPlan> {
    let face_height = face.bbox.height;
    if !face_height.is_finite() || face_height < 1.0 {
        return Err(AppError::crop_invalid());
    }

    let height = face_height / preset.face_ratio;
    let width = height * preset.width as f32 / preset.height as f32;
    let eyes = eye_center(face);
    let top = eyes.y - (1.0 - preset.eye_line_from_bottom) * height;
    let left = eyes.x - width / 2.0;
    let scale = preset.height as f32 / height;

    if !width.is_finite()
        || !height.is_finite()
        || !top.is_finite()
        || !left.is_finite()
        || !scale.is_finite()
        || scale <= 0.0
    {
        return Err(AppError::crop_invalid());
    }

    Ok(CropPlan {
        left,
        top,
        width,
        height,
        scale,
    })
}

/// Tỷ lệ chiều cao mặt trên chiều cao khung cắt (theo số đo ở ảnh nguồn).
pub fn face_height_ratio(face: &Face, plan: &CropPlan) -> f32 {
    if plan.height <= 0.0 {
        return 0.0;
    }
    face.bbox.height / plan.height
}

/// Cắt và resample khung đã tính về đúng kích thước preset, đệm màu `fill`
/// khi khung vượt ra ngoài ảnh gốc. Không kéo giãn vì scale dùng chung cho hai trục.
pub fn render(image: &RgbImage, plan: &CropPlan, preset: Preset, fill: [u8; 3]) -> RgbImage {
    let mut output = RgbImage::new(preset.width, preset.height);

    for y in 0..preset.height {
        for x in 0..preset.width {
            let source_x = plan.left + x as f32 / plan.scale;
            let source_y = plan.top + y as f32 / plan.scale;
            let pixel = sample_bilinear(image, source_x, source_y, fill);
            output.put_pixel(x, y, Rgb(pixel));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use ::image::{Rgb, RgbImage};

    use super::{face_height_ratio, plan, render};
    use crate::pipeline::face::{Face, Point, Rect};
    use crate::pipeline::preset::by_id;

    fn centered_face() -> Face {
        Face {
            bbox: Rect {
                x: 180.0,
                y: 200.0,
                width: 240.0,
                height: 300.0,
            },
            landmarks: [
                Point { x: 240.0, y: 280.0 },
                Point { x: 360.0, y: 280.0 },
                Point { x: 300.0, y: 340.0 },
                Point { x: 260.0, y: 400.0 },
                Point { x: 340.0, y: 400.0 },
            ],
            score: 0.9,
        }
    }

    #[test]
    fn plan_uses_eye_line_and_face_ratio() {
        let preset = by_id("3x4").unwrap();
        let face = centered_face();
        let crop = plan(&face, preset).unwrap();

        assert!((face_height_ratio(&face, &crop) - 0.75).abs() < 1e-6);
        assert!((crop.height - 400.0).abs() < 1e-3);
        assert!((crop.width - 400.0 * 354.0 / 472.0).abs() < 1e-3);
        assert!((crop.top - (280.0 - 0.45 * 400.0)).abs() < 1e-3);
        assert!((crop.left - (300.0 - crop.width / 2.0)).abs() < 1e-3);
    }

    #[test]
    fn render_outputs_exact_preset_size_without_stretching() {
        let preset = by_id("3x4").unwrap();
        let image = RgbImage::from_pixel(500, 500, Rgb([120, 130, 140]));
        let crop = plan(&centered_face(), preset).unwrap();
        let output = render(&image, &crop, preset, [255, 255, 255]);

        assert_eq!((output.width(), output.height()), (354, 472));
        assert_eq!(output.get_pixel(177, 236).0, [120, 130, 140]);
    }

    #[test]
    fn frame_outside_image_is_padded_with_fill_color() {
        let preset = by_id("3x4").unwrap();
        let image = RgbImage::from_pixel(100, 100, Rgb([10, 20, 30]));
        let face = Face {
            bbox: Rect {
                x: 5.0,
                y: 5.0,
                width: 40.0,
                height: 40.0,
            },
            landmarks: [
                Point { x: 15.0, y: 15.0 },
                Point { x: 25.0, y: 15.0 },
                Point { x: 20.0, y: 25.0 },
                Point { x: 15.0, y: 30.0 },
                Point { x: 25.0, y: 30.0 },
            ],
            score: 0.9,
        };
        let crop = plan(&face, preset).unwrap();
        assert!(crop.left < 0.0);
        assert!(crop.top < 0.0);

        let output = render(&image, &crop, preset, [255, 255, 255]);
        assert_eq!(output.get_pixel(0, 0).0, [255, 255, 255]);
    }
}
