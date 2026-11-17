//! Kiểm thử tích hợp cho M2: chạy trên ảnh mẫu cục bộ trong `test-images/`.
//! `test-images/` nằm trong .gitignore nên các test phụ thuộc ảnh mẫu sẽ tự bỏ
//! qua (kèm ghi chú) khi thiếu tệp.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ::image::RgbImage;

use super::face::{Face, Point, Rect};
use super::{Pipeline, ProcessOptions, process_job, select_single_face};
use crate::error::AppError;
use crate::model::JobStatus;
use crate::models::ModelStore;
use crate::queue::QueueStore;

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn models_dir() -> PathBuf {
    project_root().join("models")
}

fn test_image(name: &str) -> Option<PathBuf> {
    let path = project_root().join("test-images").join(name);
    if path.is_file() {
        Some(path)
    } else {
        eprintln!("bỏ qua test: thiếu ảnh mẫu {}", path.display());
        None
    }
}

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

fn pipeline() -> Pipeline {
    Pipeline::load(&ModelStore::new(models_dir())).expect("nạp model YuNet")
}

fn black_image(path: &Path) {
    RgbImage::new(800, 1000)
        .save(path)
        .expect("ghi ảnh đen tạm");
}

fn jfif_dpi(path: &Path) -> (u8, u16, u16) {
    let bytes = fs::read(path).expect("đọc tệp JPG");
    let position = bytes
        .windows(5)
        .position(|window| window == b"JFIF\0")
        .expect("phải có JFIF header");
    let units = bytes[position + 7];
    let x = u16::from_be_bytes([bytes[position + 8], bytes[position + 9]]);
    let y = u16::from_be_bytes([bytes[position + 10], bytes[position + 11]]);
    (units, x, y)
}

#[test]
fn normal_photo_exports_one_face_at_contract_size() {
    let Some(source) = test_image("img 1 normal.jpg") else {
        return;
    };
    let output_dir = unique_dir("pipeline-normal");
    let mut pipeline = pipeline();
    let options = ProcessOptions::default();

    let report = pipeline
        .process(&source, &output_dir, &options)
        .expect("xử lý ảnh bình thường");

    println!(
        "img 1 normal.jpg: faces={} score={:.4} angle={:.3}° ratio={:.4} residual={:.4}° output={:?}",
        report.face_count,
        report.face_score,
        report.eye_angle_deg,
        report.face_height_ratio,
        report.residual_angle_deg,
        report.output_path.file_name()
    );

    assert_eq!(report.face_count, 1, "ảnh mẫu phải có đúng 1 khuôn mặt");
    assert_eq!(report.preset_id, "3x4");
    assert_eq!((report.output_width, report.output_height), (354, 472));
    assert_eq!(
        report
            .output_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("img_1_normal_3x4_trang.jpg")
    );
    assert!(report.output_path.is_file());
    assert!(
        (0.70..=0.80).contains(&report.face_height_ratio),
        "tỷ lệ mặt theo kế hoạch ngoài 70–80%: {}",
        report.face_height_ratio
    );

    let (units, x_dpi, y_dpi) = jfif_dpi(&report.output_path);
    assert_eq!(units, 1, "JFIF phải ghi đơn vị DPI");
    assert_eq!((x_dpi, y_dpi), (300, 300));

    let exported = ::image::open(&report.output_path)
        .expect("đọc lại ảnh đã xuất")
        .to_rgb8();
    assert_eq!(exported.dimensions(), (354, 472));

    let faces = pipeline.detector.detect(&exported).expect("nhận diện lại");
    assert_eq!(faces.len(), 1, "ảnh xuất phải còn đúng 1 khuôn mặt");
    let ratio = faces[0].bbox.height / exported.height() as f32;
    println!("tỷ lệ mặt đo lại trên ảnh xuất: {ratio:.4}");
    assert!(
        (0.70..=0.80).contains(&ratio),
        "tỷ lệ mặt đo lại ngoài 70–80%: {ratio}"
    );

    assert!(!output_dir.join("img_1_normal_3x4_trang.jpg.part").exists());
}

#[test]
fn every_m2_preset_exports_its_own_pixel_size() {
    let Some(source) = test_image("img 1 normal.jpg") else {
        return;
    };
    let output_dir = unique_dir("pipeline-presets");
    let mut pipeline = pipeline();
    let expected = [
        ("2x3", 236_u32, 354_u32),
        ("3x4", 354, 472),
        ("4x6", 472, 709),
        ("3.5x4.5", 413, 531),
    ];

    for (id, width, height) in expected {
        let options = ProcessOptions {
            preset_id: id.to_string(),
            ..ProcessOptions::default()
        };
        let report = pipeline
            .process(&source, &output_dir, &options)
            .unwrap_or_else(|error| panic!("preset {id} lỗi: {}", error.code));

        println!(
            "preset {id}: {}x{}",
            report.output_width, report.output_height
        );
        assert_eq!((report.output_width, report.output_height), (width, height));
        let exported = ::image::open(&report.output_path).expect("đọc ảnh xuất");
        assert_eq!(exported.width(), width);
        assert_eq!(exported.height(), height);
    }
}

#[test]
fn unsupported_preset_is_rejected_before_loading() {
    let Some(source) = test_image("img 1 normal.jpg") else {
        return;
    };
    let output_dir = unique_dir("pipeline-5x5");
    let mut pipeline = pipeline();
    let options = ProcessOptions {
        preset_id: "5x5".to_string(),
        ..ProcessOptions::default()
    };

    let error = pipeline
        .process(&source, &output_dir, &options)
        .expect_err("preset 5x5 phải bị từ chối ở M2");
    assert_eq!(error.code, "E006");
}

#[test]
fn corrupt_image_returns_e001() {
    let dir = unique_dir("pipeline-corrupt");
    let source = dir.join("hong.jpg");
    fs::write(&source, b"khong-phai-anh").expect("ghi tệp hỏng");
    let output_dir = unique_dir("pipeline-corrupt-out");
    let mut pipeline = pipeline();

    let error = pipeline
        .process(&source, &output_dir, &ProcessOptions::default())
        .expect_err("tệp hỏng phải lỗi");
    assert_eq!(error.code, "E001");
}

#[test]
fn image_without_face_returns_e002() {
    let dir = unique_dir("pipeline-no-face");
    let source = dir.join("den.png");
    black_image(&source);
    let output_dir = unique_dir("pipeline-no-face-out");
    let mut pipeline = pipeline();

    let error = pipeline
        .process(&source, &output_dir, &ProcessOptions::default())
        .expect_err("ảnh đen phải không có khuôn mặt");
    assert_eq!(error.code, "E002");
}

#[test]
fn select_single_face_rejects_zero_and_many() {
    let face = Face {
        bbox: Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        landmarks: [Point { x: 0.0, y: 0.0 }; 5],
        score: 0.9,
    };

    assert_eq!(
        select_single_face(Vec::new()).unwrap_err().code,
        AppError::face_not_found().code
    );
    assert_eq!(
        select_single_face(vec![face.clone(), face])
            .unwrap_err()
            .code,
        AppError::multiple_faces().code
    );
}

#[test]
fn bad_model_checksum_returns_e004() {
    let dir = unique_dir("pipeline-bad-model");
    fs::write(dir.join("model.onnx"), b"data").expect("ghi model giả");
    fs::write(
        dir.join("manifest.json"),
        r#"{"version":1,"models":[{"id":"yunet","file":"model.onnx","size":4,"sha256":"0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
    )
    .expect("ghi manifest giả");

    let error = Pipeline::load(&ModelStore::new(dir))
        .err()
        .expect("SHA sai phải bị chặn");
    assert_eq!(error.code, "E004");
}

#[test]
fn process_job_updates_queue_to_da_xuat_and_loi() {
    let Some(source) = test_image("img 1 normal.jpg") else {
        return;
    };
    let dir = unique_dir("pipeline-queue");
    let output_dir = dir.join("out");
    fs::create_dir_all(&output_dir).expect("tạo thư mục đầu ra");
    let black = dir.join("den.png");
    black_image(&black);

    let mut queue = QueueStore::load(dir.join("queue.json"));
    let good = queue
        .add_stable_file(&source, "sha-normal-queue")
        .expect("thêm job ảnh tốt")
        .expect("job ảnh tốt");
    let bad = queue
        .add_stable_file(&black, "sha-black-queue")
        .expect("thêm job ảnh đen")
        .expect("job ảnh đen");

    let mut pipeline = pipeline();
    let report = process_job(
        &mut queue,
        &mut pipeline,
        &good.id,
        &output_dir,
        &ProcessOptions::default(),
    )
    .expect("job ảnh tốt phải xuất được");
    assert!(report.output_path.is_file());

    let error = process_job(
        &mut queue,
        &mut pipeline,
        &bad.id,
        &output_dir,
        &ProcessOptions::default(),
    )
    .expect_err("job ảnh đen phải lỗi");
    assert_eq!(error.code, "E002");

    let state = queue.state().expect("đọc trạng thái hàng đợi");
    let good_job = state
        .jobs
        .iter()
        .find(|job| job.id == good.id)
        .expect("job tốt còn trong hàng đợi");
    let bad_job = state
        .jobs
        .iter()
        .find(|job| job.id == bad.id)
        .expect("job lỗi còn trong hàng đợi");

    assert_eq!(good_job.status, JobStatus::DaXuat);
    assert_eq!(
        good_job.output_path.as_deref(),
        Some(report.output_path.to_string_lossy().as_ref())
    );
    assert_eq!(bad_job.status, JobStatus::Loi);
    assert_eq!(bad_job.reason_code.as_deref(), Some("E002"));
    assert!(bad_job.output_path.is_none());
    assert_eq!(state.done, 2);
}
