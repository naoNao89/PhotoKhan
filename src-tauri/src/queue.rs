use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock;
use crate::error::{AppError, AppResult};
use crate::model::{Job, JobStatus, QueueState};
use crate::persistence::{atomic_write_json, load_json};

const QUEUE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueueRecord {
    #[serde(flatten)]
    job: Job,
    source_path: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueueFile {
    #[serde(default = "queue_version")]
    version: u32,
    #[serde(default)]
    paused: bool,
    #[serde(default)]
    jobs: Vec<QueueRecord>,
}

fn queue_version() -> u32 {
    QUEUE_VERSION
}

fn job_not_found() -> AppError {
    AppError::new(
        "QUEUE_IO",
        "Không tìm thấy ảnh trong hàng đợi.",
        "Hãy tải lại danh sách ảnh rồi thử lại.",
    )
}

/// Hàng đợi bền vững. Mọi thay đổi đều được ghi ngay xuống `queue.json`.
pub struct QueueStore {
    path: PathBuf,
    records: Vec<QueueRecord>,
    paused: bool,
    load_error: Option<AppError>,
}

impl QueueStore {
    pub fn load(path: PathBuf) -> Self {
        let mut store = Self {
            path,
            records: Vec::new(),
            paused: false,
            load_error: None,
        };

        let loaded = load_json::<QueueFile>(
            &store.path,
            "QUEUE_IO",
            AppError::queue_corrupt(),
            "queue.json",
        );

        match loaded {
            Ok(Some(file)) => {
                store.records = file.jobs;
                store.paused = file.paused;
                if let Err(error) = store.normalize_after_restart() {
                    store.load_error = Some(error);
                }
            }
            Ok(None) => {}
            Err(error) => store.load_error = Some(error),
        }

        store
    }

    pub fn state(&self) -> AppResult<QueueState> {
        self.ensure_available()?;
        let done = self
            .records
            .iter()
            .filter(|record| record.job.status.is_done())
            .count();

        Ok(QueueState {
            jobs: self
                .records
                .iter()
                .map(|record| record.job.clone())
                .collect(),
            done,
            total: self.records.len(),
            paused: self.paused,
        })
    }

    /// Thêm một tệp đã ổn định. Trả về `None` nếu SHA-256 đã có trong hàng đợi.
    pub fn add_stable_file(&mut self, source_path: &Path, sha256: &str) -> AppResult<Option<Job>> {
        self.ensure_available()?;

        if sha256.trim().is_empty() {
            return Err(AppError::new(
                "QUEUE_IO",
                "Không tính được mã kiểm tra của tệp ảnh.",
                "Hãy kiểm tra quyền đọc tệp ảnh rồi thử lại.",
            ));
        }

        if self.records.iter().any(|record| record.sha256 == sha256) {
            return Ok(None);
        }

        let file_name = source_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "khong-ro-ten.jpg".to_string());
        let now = clock::now_iso8601()?;
        let job = Job {
            id: format!("job-{sha256}"),
            file_name,
            status: JobStatus::Cho,
            reason_code: None,
            reason_text: None,
            output_path: None,
            updated_at: now,
        };

        self.records.push(QueueRecord {
            job: job.clone(),
            source_path: source_path.to_string_lossy().into_owned(),
            sha256: sha256.to_string(),
        });

        if let Err(error) = self.save() {
            self.records.pop();
            return Err(error);
        }

        Ok(Some(job))
    }

    pub fn set_paused(&mut self, paused: bool) -> AppResult<()> {
        self.ensure_available()?;
        if self.paused == paused {
            return Ok(());
        }

        let previous = self.paused;
        self.paused = paused;
        if let Err(error) = self.save() {
            self.paused = previous;
            return Err(error);
        }

        Ok(())
    }

    /// Đường dẫn tệp gốc của một job (không lộ ra ngoài hàng đợi/giao diện).
    #[allow(dead_code)]
    pub fn source_path(&self, id: &str) -> AppResult<PathBuf> {
        self.ensure_available()?;
        self.records
            .iter()
            .find(|record| record.job.id == id)
            .map(|record| PathBuf::from(&record.source_path))
            .ok_or_else(job_not_found)
    }

    /// Bắt đầu xử lý: `DANG_XU_LY`, xóa lý do lỗi cũ và đường dẫn đầu ra cũ.
    #[allow(dead_code)]
    pub fn begin_job(&mut self, id: &str) -> AppResult<Job> {
        self.update_job(id, |job, now| {
            job.status = JobStatus::DangXuLy;
            job.reason_code = None;
            job.reason_text = None;
            job.output_path = None;
            job.updated_at = now.to_string();
        })
    }

    /// Xuất thành công: `DA_XUAT` kèm đường dẫn tệp đầu ra.
    #[allow(dead_code)]
    pub fn finish_exported(&mut self, id: &str, output_path: &Path) -> AppResult<Job> {
        let output_path = output_path.to_string_lossy().into_owned();
        self.update_job(id, |job, now| {
            job.status = JobStatus::DaXuat;
            job.reason_code = None;
            job.reason_text = None;
            job.output_path = Some(output_path.clone());
            job.updated_at = now.to_string();
        })
    }

    /// Xử lý lỗi: `LOI` kèm mã và thông điệp tiếng Việt từ `AppError`.
    #[allow(dead_code)]
    pub fn fail_job(&mut self, id: &str, error: &AppError) -> AppResult<Job> {
        let code = error.code.clone();
        let message = error.message_vi.clone();
        self.update_job(id, |job, now| {
            job.status = JobStatus::Loi;
            job.reason_code = Some(code.clone());
            job.reason_text = Some(message.clone());
            job.output_path = None;
            job.updated_at = now.to_string();
        })
    }

    /// Cập nhật trạng thái một job. Dùng cho kiểm thử khôi phục ở M1 và cho
    /// các milestone sau.
    #[cfg(test)]
    pub fn set_status(&mut self, id: &str, status: JobStatus) -> AppResult<Job> {
        self.update_job(id, |job, now| {
            job.status = status;
            job.updated_at = now.to_string();
        })
    }

    /// Áp thay đổi lên một job và ghi ngay xuống đĩa; hoàn tác nếu ghi lỗi.
    #[allow(dead_code)]
    fn update_job<F>(&mut self, id: &str, apply: F) -> AppResult<Job>
    where
        F: FnOnce(&mut Job, &str),
    {
        self.ensure_available()?;
        let now = clock::now_iso8601()?;
        let index = self
            .records
            .iter()
            .position(|record| record.job.id == id)
            .ok_or_else(job_not_found)?;

        let previous = self.records[index].job.clone();
        apply(&mut self.records[index].job, &now);

        if let Err(error) = self.save() {
            self.records[index].job = previous;
            return Err(error);
        }

        Ok(self.records[index].job.clone())
    }

    #[cfg(test)]
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    fn ensure_available(&self) -> AppResult<()> {
        match &self.load_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn save(&self) -> AppResult<()> {
        let file = QueueFile {
            version: QUEUE_VERSION,
            paused: self.paused,
            jobs: self.records.clone(),
        };
        atomic_write_json(&self.path, &file, "QUEUE_IO", "queue.json")
    }

    /// `DANG_XU_LY` trở về `CHO` sau khi khởi động lại; `DA_XUAT` giữ nguyên.
    fn normalize_after_restart(&mut self) -> AppResult<()> {
        if !self
            .records
            .iter()
            .any(|record| record.job.status == JobStatus::DangXuLy)
        {
            return Ok(());
        }

        let now = clock::now_iso8601()?;
        for record in &mut self.records {
            if record.job.status == JobStatus::DangXuLy {
                record.job.status = JobStatus::Cho;
                record.job.updated_at = now.clone();
            }
        }

        self.save()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::model::JobStatus;

    use super::QueueStore;

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

    fn source_path(name: &str) -> PathBuf {
        Path::new("C:\\anh").join(name)
    }

    #[test]
    fn saves_and_reloads_with_a_new_store_object() {
        let dir = unique_dir("queue-round-trip");
        let path = dir.join("queue.json");
        let mut store = QueueStore::load(path.clone());

        let job = store
            .add_stable_file(&source_path("be.jpg"), "sha-be")
            .unwrap()
            .unwrap();
        store.set_paused(true).unwrap();

        let reloaded = QueueStore::load(path);
        let state = reloaded.state().unwrap();

        assert_eq!(state.total, 1);
        assert_eq!(state.done, 0);
        assert!(state.paused);
        assert_eq!(state.jobs[0], job);
    }

    #[test]
    fn restart_converts_in_progress_to_waiting_and_keeps_exported() {
        let dir = unique_dir("queue-recovery");
        let path = dir.join("queue.json");
        let mut store = QueueStore::load(path.clone());

        let first = store
            .add_stable_file(&source_path("mot.jpg"), "sha-mot")
            .unwrap()
            .unwrap();
        let second = store
            .add_stable_file(&source_path("hai.jpg"), "sha-hai")
            .unwrap()
            .unwrap();
        store.set_status(&first.id, JobStatus::DangXuLy).unwrap();
        store.set_status(&second.id, JobStatus::DaXuat).unwrap();

        let reloaded = QueueStore::load(path);
        let state = reloaded.state().unwrap();
        let first_after = state.jobs.iter().find(|job| job.id == first.id).unwrap();
        let second_after = state.jobs.iter().find(|job| job.id == second.id).unwrap();

        assert_eq!(first_after.status, JobStatus::Cho);
        assert_eq!(second_after.status, JobStatus::DaXuat);
    }

    #[test]
    fn corrupt_queue_returns_an_error_instead_of_panicking() {
        let dir = unique_dir("queue-corrupt");
        let path = dir.join("queue.json");
        fs::write(&path, b"{not-json").unwrap();

        let store = QueueStore::load(path);
        let error = store.state().unwrap_err();

        assert_eq!(error.code, "QUEUE_CORRUPT");
    }

    #[test]
    fn watcher_style_duplicate_hash_is_added_only_once() {
        let dir = unique_dir("queue-dedupe");
        let path = dir.join("queue.json");
        let mut store = QueueStore::load(path);

        assert!(
            store
                .add_stable_file(&source_path("be.jpg"), "sha-giong-nhau")
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .add_stable_file(&source_path("be-copy.jpg"), "sha-giong-nhau")
                .unwrap()
                .is_none()
        );
        assert_eq!(store.record_count(), 1);
    }

    #[test]
    fn queue_state_does_not_leak_internal_fields() {
        let dir = unique_dir("queue-shape");
        let path = dir.join("queue.json");
        let mut store = QueueStore::load(path);
        store
            .add_stable_file(&source_path("be.jpg"), "sha-shape")
            .unwrap();

        let value = serde_json::to_value(store.state().unwrap()).unwrap();
        let job = &value["jobs"][0];

        assert!(job.get("source_path").is_none());
        assert!(job.get("sha256").is_none());
        assert_eq!(job["status"], "CHO");
    }
}
