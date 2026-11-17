//! Đọc `models/manifest.json` và xác minh SHA-256 trước khi nạp model.
//! M2 chưa nối lệnh Tauri (D1) nên tạm cho phép dead_code.
#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

pub const YUNET_MODEL_ID: &str = "yunet";
pub const MANIFEST_FILE_NAME: &str = "manifest.json";

/// Một mục trong `models/manifest.json` (SRS mục 7).
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct ModelEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    file: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    size: u64,
    sha256: String,
    #[serde(default)]
    license: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

/// Quản lý model cục bộ: chỉ đọc manifest và kiểm tra SHA-256, không tải qua mạng.
#[derive(Debug, Clone)]
pub struct ModelStore {
    dir: PathBuf,
}

impl ModelStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Vị trí khi chạy thật: thư mục `models\` cạnh file thực thi (CONTRACT.md).
    /// Chưa dùng ở M2 vì chưa nối hàng đợi; sẽ dùng khi bọc lệnh xử lý ở bước sau.
    #[allow(dead_code)]
    pub fn beside_executable() -> AppResult<Self> {
        let executable = std::env::current_exe().map_err(|_| AppError::model_invalid())?;
        let dir = executable
            .parent()
            .ok_or_else(AppError::model_invalid)?
            .join("models");
        Ok(Self::new(dir))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Trả đường dẫn YuNet sau khi đã xác minh manifest, kích thước và SHA-256.
    pub fn yunet_path(&self) -> AppResult<PathBuf> {
        self.verified_path(YUNET_MODEL_ID)
    }

    fn verified_path(&self, id: &str) -> AppResult<PathBuf> {
        let manifest_path = self.dir.join(MANIFEST_FILE_NAME);
        let bytes = fs::read(&manifest_path).map_err(|_| AppError::model_invalid())?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| AppError::model_invalid())?;

        let entry = manifest
            .models
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(AppError::model_invalid)?;

        let file_path = Path::new(&entry.file);
        let mut components = file_path.components();
        let only_component = components.next();
        if components.next().is_some()
            || !matches!(only_component, Some(Component::Normal(_)))
            || file_path.is_absolute()
        {
            return Err(AppError::model_invalid());
        }

        let model_path = self.dir.join(file_path);
        let metadata = fs::metadata(&model_path).map_err(|_| AppError::model_invalid())?;
        if !metadata.is_file() {
            return Err(AppError::model_invalid());
        }

        if entry.size != 0 && metadata.len() != entry.size {
            return Err(AppError::model_invalid());
        }

        let actual = sha256_file(&model_path)?;
        if !actual.eq_ignore_ascii_case(entry.sha256.trim()) {
            return Err(AppError::model_invalid());
        }

        Ok(model_path)
    }
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let file = File::open(path).map_err(|_| AppError::model_invalid())?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| AppError::model_invalid())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{MANIFEST_FILE_NAME, ModelStore};
    use crate::error::AppResult;

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

    fn write_manifest(dir: &std::path::Path, sha256: &str) -> AppResult<()> {
        let manifest = format!(
            r#"{{"version":1,"models":[{{"id":"yunet","name":"YuNet","version":"2023mar","file":"model.onnx","url":"","size":4,"sha256":"{sha256}","license":"MIT"}}]}}"#
        );
        fs::write(dir.join(MANIFEST_FILE_NAME), manifest)
            .map_err(|_| crate::error::AppError::model_invalid())
    }

    #[test]
    fn missing_manifest_returns_e004() {
        let dir = unique_dir("model-no-manifest");
        let error = ModelStore::new(dir).yunet_path().unwrap_err();
        assert_eq!(error.code, "E004");
    }

    #[test]
    fn wrong_sha256_returns_e004() {
        let dir = unique_dir("model-bad-sha");
        fs::write(dir.join("model.onnx"), b"data").unwrap();
        write_manifest(
            &dir,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        let error = ModelStore::new(dir).yunet_path().unwrap_err();
        assert_eq!(error.code, "E004");
    }

    #[test]
    fn matching_sha256_is_accepted() {
        let dir = unique_dir("model-good-sha");
        fs::write(dir.join("model.onnx"), b"data").unwrap();
        write_manifest(
            &dir,
            "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7",
        )
        .unwrap();

        let path = ModelStore::new(dir).yunet_path().unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn missing_model_file_returns_e004() {
        let dir = unique_dir("model-missing-file");
        write_manifest(
            &dir,
            "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7",
        )
        .unwrap();

        let error = ModelStore::new(dir).yunet_path().unwrap_err();
        assert_eq!(error.code, "E004");
    }
}
