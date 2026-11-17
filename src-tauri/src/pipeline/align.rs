use ::image::{Rgb, RgbImage};

use super::face::{Face, Point, Rect};

/// Góc của đường nối hai mắt so với phương ngang, đơn vị độ, trong khoảng [-90, 90].
pub fn eye_angle_deg(face: &Face) -> f32 {
    let right = face.right_eye();
    let left = face.left_eye();
    let mut degrees = (left.y - right.y).atan2(left.x - right.x).to_degrees();

    while degrees > 90.0 {
        degrees -= 180.0;
    }
    while degrees < -90.0 {
        degrees += 180.0;
    }
    degrees
}

pub fn eye_center(face: &Face) -> Point {
    midpoint(face.right_eye(), face.left_eye())
}

pub fn midpoint(left: Point, right: Point) -> Point {
    Point {
        x: (left.x + right.x) / 2.0,
        y: (left.y + right.y) / 2.0,
    }
}

/// Xoay ảnh quanh tâm hai mắt (nội suy song tuyến) và biến đổi landmark theo.
/// Trả về ảnh đã căn thẳng và khuôn mặt với toạ độ mới. Kích thước bbox được giữ
/// nguyên (chỉ xoay tâm) để tỷ lệ chiều cao mặt không bị phóng đại.
pub fn straighten(
    image: &RgbImage,
    face: &Face,
    angle_deg: f32,
    fill: [u8; 3],
) -> (RgbImage, Face) {
    let center = eye_center(face);
    let rotated_image = rotate_about(image, center, angle_deg, fill);

    let mut rotated_face = face.clone();
    for landmark in &mut rotated_face.landmarks {
        *landmark = rotate_point(*landmark, center, -angle_deg);
    }

    let bbox_center = Point {
        x: face.bbox.x + face.bbox.width / 2.0,
        y: face.bbox.y + face.bbox.height / 2.0,
    };
    let rotated_center = rotate_point(bbox_center, center, -angle_deg);
    rotated_face.bbox = Rect {
        x: rotated_center.x - face.bbox.width / 2.0,
        y: rotated_center.y - face.bbox.height / 2.0,
        width: face.bbox.width,
        height: face.bbox.height,
    };

    (rotated_image, rotated_face)
}

/// Xoay một điểm quanh tâm theo chiều kim đồng hồ khi góc dương (trục y hướng xuống).
pub fn rotate_point(point: Point, center: Point, degrees: f32) -> Point {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Point {
        x: center.x + cos * dx - sin * dy,
        y: center.y + sin * dx + cos * dy,
    }
}

fn rotate_about(image: &RgbImage, center: Point, degrees: f32, fill: [u8; 3]) -> RgbImage {
    if degrees == 0.0 {
        return image.clone();
    }

    let (width, height) = image.dimensions();
    let mut output = RgbImage::from_pixel(width, height, Rgb(fill));

    for y in 0..height {
        for x in 0..width {
            let destination = Point {
                x: x as f32,
                y: y as f32,
            };
            let source = rotate_point(destination, center, degrees);
            let pixel = sample_bilinear(image, source.x, source.y, fill);
            output.put_pixel(x, y, Rgb(pixel));
        }
    }

    output
}

/// Lấy mẫu song tuyến. Điểm ngoài ảnh dùng màu đệm.
pub(crate) fn sample_bilinear(image: &RgbImage, x: f32, y: f32, fill: [u8; 3]) -> [u8; 3] {
    if !x.is_finite() || !y.is_finite() {
        return fill;
    }

    let max_x = image.width() as f32;
    let max_y = image.height() as f32;
    if x < -1.0 || y < -1.0 || x > max_x || y > max_y {
        return fill;
    }

    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let x0 = x0 as i64;
    let y0 = y0 as i64;

    let c00 = pixel_or_fill(image, x0, y0, fill);
    let c10 = pixel_or_fill(image, x0 + 1, y0, fill);
    let c01 = pixel_or_fill(image, x0, y0 + 1, fill);
    let c11 = pixel_or_fill(image, x0 + 1, y0 + 1, fill);

    let weights = [
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    ];

    let mut result = [0_u8; 3];
    for channel in 0..3 {
        let value = c00[channel] * weights[0]
            + c10[channel] * weights[1]
            + c01[channel] * weights[2]
            + c11[channel] * weights[3];
        result[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    result
}

fn pixel_or_fill(image: &RgbImage, x: i64, y: i64, fill: [u8; 3]) -> [f32; 3] {
    if x < 0 || y < 0 || x >= i64::from(image.width()) || y >= i64::from(image.height()) {
        return fill.map(f32::from);
    }

    let pixel = image.get_pixel(x as u32, y as u32).0;
    [
        f32::from(pixel[0]),
        f32::from(pixel[1]),
        f32::from(pixel[2]),
    ]
}

#[cfg(test)]
mod tests {
    use ::image::{Rgb, RgbImage};

    use super::{eye_angle_deg, rotate_point, sample_bilinear, straighten};
    use crate::pipeline::face::{Face, Point, Rect};

    fn face_with_eyes(right: Point, left: Point) -> Face {
        Face {
            bbox: Rect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 50.0,
            },
            landmarks: [
                right,
                left,
                Point { x: 30.0, y: 40.0 },
                Point { x: 20.0, y: 50.0 },
                Point { x: 40.0, y: 50.0 },
            ],
            score: 0.9,
        }
    }

    #[test]
    fn level_eyes_have_zero_angle() {
        let face = face_with_eyes(Point { x: 20.0, y: 30.0 }, Point { x: 40.0, y: 30.0 });
        assert!(eye_angle_deg(&face).abs() < 1e-5);
    }

    #[test]
    fn tilted_eyes_report_signed_angle() {
        let face = face_with_eyes(Point { x: 20.0, y: 30.0 }, Point { x: 40.0, y: 40.0 });
        assert!((eye_angle_deg(&face) - 26.565).abs() < 0.01);
    }

    #[test]
    fn rotating_point_around_center_is_reversible() {
        let center = Point { x: 5.0, y: 5.0 };
        let point = Point { x: 7.0, y: 3.0 };
        let once = rotate_point(point, center, 17.0);
        let back = rotate_point(once, center, -17.0);
        assert!((back.x - point.x).abs() < 1e-4);
        assert!((back.y - point.y).abs() < 1e-4);
    }

    #[test]
    fn straighten_makes_eye_line_level() {
        let image = RgbImage::from_pixel(100, 100, Rgb([10, 20, 30]));
        let face = face_with_eyes(Point { x: 30.0, y: 40.0 }, Point { x: 50.0, y: 50.0 });
        let (_, rotated) = straighten(&image, &face, eye_angle_deg(&face), [255, 255, 255]);
        assert!(eye_angle_deg(&rotated).abs() < 1e-4);
    }

    #[test]
    fn bilinear_sampling_returns_fill_outside_image() {
        let image = RgbImage::from_pixel(10, 10, Rgb([10, 20, 30]));
        assert_eq!(sample_bilinear(&image, -5.0, 5.0, [255, 255, 255]), [255, 255, 255]);
        assert_eq!(sample_bilinear(&image, 5.0, 5.0, [255, 255, 255]), [10, 20, 30]);
    }
}
