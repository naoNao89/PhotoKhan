use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{AppError, AppResult};

/// Đọc JSON từ đĩa. Trả về `None` nếu tệp chưa tồn tại.
pub fn load_json<T: DeserializeOwned>(
    path: &Path,
    io_code: &str,
    corrupt_error: AppError,
    label: &str,
) -> AppResult<Option<T>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(io_error(io_code, label)),
    };

    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| corrupt_error)
}

/// Ghi JSON nguyên tử: ghi vào `<tên>.tmp`, đồng bộ xuống đĩa, rồi đổi tên.
pub fn atomic_write_json<T: Serialize>(
    path: &Path,
    value: &T,
    io_code: &str,
    label: &str,
) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| io_error(io_code, label))?;
    }

    let temp_path = temp_path_for(path, io_code, label)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| io_error(io_code, label))?;

    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(io_code, label));
    }

    if fs::rename(&temp_path, path).is_err() {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(io_code, label));
    }

    Ok(())
}

fn temp_path_for(path: &Path, io_code: &str, label: &str) -> AppResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io_error(io_code, label))?;

    Ok(path.with_file_name(format!("{file_name}.tmp")))
}

fn io_error(code: &str, label: &str) -> AppError {
    match code {
        "QUEUE_IO" => AppError::queue_io(),
        "SETTINGS_IO" => AppError::settings_io(),
        "WATCHER_IO" => AppError::watcher_io(),
        _ => AppError::new(
            code,
            format!("Không đọc hoặc ghi được {label}."),
            "Hãy kiểm tra quyền truy cập thư mục dữ liệu của ứng dụng rồi thử lại.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::atomic_write_json;

    fn unique_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "photokhan-test-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_write_replaces_existing_file_without_temp_leftover() {
        let dir = unique_dir("atomic");
        let path = dir.join("queue.json");
        fs::write(&path, b"old").unwrap();

        atomic_write_json(&path, &vec!["new"], "QUEUE_IO", "queue.json").unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("new"));
        assert!(!dir.join("queue.json.tmp").exists());
    }
}
