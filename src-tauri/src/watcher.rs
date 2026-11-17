use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::model::{Job, Settings};
use crate::queue::QueueStore;
use crate::state::AppState;

const SUPPORTED_EXTENSIONS: [&str; 9] = [
    "jpg", "jpeg", "png", "heic", "heif", "bmp", "tif", "tiff", "webp",
];
const WAIT_SLICE: Duration = Duration::from_millis(200);

pub struct WatcherManager {
    runtime: Option<WatcherRuntime>,
}

struct WatcherRuntime {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl WatcherManager {
    pub fn new() -> Self {
        Self { runtime: None }
    }

    /// Khởi động lại watcher theo cài đặt mới. Nếu không có thư mục input,
    /// watcher cũ được dừng và không có watcher mới.
    pub fn restart(&mut self, app: AppHandle, settings: &Settings) -> AppResult<()> {
        self.stop()?;

        let input = match settings
            .input_dir
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(value) => PathBuf::from(value),
            None => return Ok(()),
        };

        if !input.is_dir() {
            return Err(AppError::input_dir_invalid());
        }

        let interval = Duration::from_secs(settings.poll_interval_secs.max(1));
        self.start(app, input, interval)
    }

    pub fn stop(&mut self) -> AppResult<()> {
        if let Some(mut runtime) = self.runtime.take() {
            runtime.stop.store(true, Ordering::SeqCst);
            if let Some(join) = runtime.join.take()
                && join.join().is_err()
            {
                return Err(AppError::watcher_io());
            }
        }
        Ok(())
    }

    fn start(&mut self, app: AppHandle, input: PathBuf, interval: Duration) -> AppResult<()> {
        let (event_tx, event_rx) = std::sync::mpsc::sync_channel::<()>(1);
        let watcher = build_notify_watcher(&input, event_tx);

        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_input = input.clone();
        let join = thread::Builder::new()
            .name("photokhan-watcher".to_string())
            .spawn(move || {
                watch_loop(app, thread_input, interval, watcher, event_rx, thread_stop);
            })
            .map_err(|_| AppError::watcher_io())?;

        self.runtime = Some(WatcherRuntime {
            stop,
            join: Some(join),
        });
        Ok(())
    }
}

/// `notify` có thể không theo dõi được ổ mạng hoặc thư mục đồng bộ. Trong
/// trường hợp đó watcher vẫn chạy bằng polling theo chu kỳ cấu hình.
fn build_notify_watcher(
    input: &Path,
    event_tx: std::sync::mpsc::SyncSender<()>,
) -> Option<RecommendedWatcher> {
    let mut watcher = match RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| match result {
            Ok(_) => {
                let _ = event_tx.try_send(());
            }
            Err(_) => {
                tracing::warn!("Watcher nhận sự kiện hệ thống bị lỗi.");
            }
        },
        notify::Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(_) => {
            tracing::warn!("Không tạo được watcher hệ thống; chuyển sang polling.");
            return None;
        }
    };

    if watcher.watch(input, RecursiveMode::NonRecursive).is_err() {
        tracing::warn!("Không theo dõi được thư mục bằng sự kiện; chuyển sang polling.");
        return None;
    }

    Some(watcher)
}

impl Drop for WatcherManager {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::warn!(
                code = %error.code,
                "Không dừng sạch được watcher: {}",
                error.message_vi
            );
        }
    }
}

fn watch_loop(
    app: AppHandle,
    input: PathBuf,
    interval: Duration,
    watcher: Option<RecommendedWatcher>,
    event_rx: std::sync::mpsc::Receiver<()>,
    stop: Arc<AtomicBool>,
) {
    let mut tracker = ScanTracker::new();
    let mut next_scan = Instant::now();

    while !stop.load(Ordering::SeqCst) {
        if watcher.is_some() {
            let _ = event_rx.recv_timeout(WAIT_SLICE);
        } else {
            thread::sleep(WAIT_SLICE);
        }

        if Instant::now() < next_scan {
            continue;
        }

        let state = app.state::<AppState>();
        let mut queue = match state.queue.lock() {
            Ok(queue) => queue,
            Err(_) => {
                tracing::warn!("Không truy cập được hàng đợi khi quét thư mục.");
                next_scan = Instant::now() + interval;
                continue;
            }
        };

        let result = scan_and_enqueue(&input, &mut tracker, &mut queue, |job| {
            tracing::info!(job_id = %job.id, "Đã thêm ảnh vào hàng đợi.");
            if let Err(error) = app.emit("job_updated", job) {
                tracing::warn!(
                    job_id = %job.id,
                    "Không gửi được sự kiện job_updated: {}",
                    error
                );
            }
        });
        drop(queue);

        if let Err(error) = result {
            tracing::warn!(
                code = %error.code,
                "Quét thư mục ảnh gốc thất bại: {}",
                error.message_vi
            );
        }

        next_scan = Instant::now() + interval;
    }
}

/// Quét một lần, chỉ nhận file có kích thước ổn định qua hai lần quét.
pub(crate) fn scan_and_enqueue<F>(
    input: &Path,
    tracker: &mut ScanTracker,
    queue: &mut QueueStore,
    mut on_job: F,
) -> AppResult<Vec<Job>>
where
    F: FnMut(&Job),
{
    let stable_files = tracker.scan(input)?;
    let mut added = Vec::new();
    let mut first_error = None;

    for stable in stable_files {
        let sha256 = match sha256_file(&stable.path) {
            Ok(sha256) => sha256,
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
        };

        match queue.add_stable_file(&stable.path, &sha256) {
            Ok(Some(job)) => {
                tracker.mark_accepted(&stable);
                added.push(job.clone());
                on_job(&job);
            }
            Ok(None) => {
                // SHA-256 đã có trong hàng đợi: đánh dấu đã xử lý để không hash lại.
                tracker.mark_accepted(&stable);
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    if let Some(error) = first_error {
        Err(error)
    } else {
        Ok(added)
    }
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = fs::File::open(path).map_err(|_| {
        AppError::new(
            "E001",
            "Không đọc được tệp ảnh.",
            "Hãy kiểm tra quyền truy cập tệp ảnh rồi thử lại.",
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file.read(&mut buffer).map_err(|_| {
            AppError::new(
                "E001",
                "Không đọc được tệp ảnh.",
                "Hãy kiểm tra quyền truy cập tệp ảnh rồi thử lại.",
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let sha256 = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(sha256)
}

#[derive(Debug, Clone)]
pub(crate) struct StableFile {
    path: PathBuf,
    fingerprint: Fingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fingerprint {
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
struct Observation {
    size: u64,
    count: u8,
}

/// Theo dõi kích thước file qua các lần quét. Chỉ trả về file khi kích thước
/// giống nhau ở hai lần quét liên tiếp và file chưa được nhận trước đó.
#[derive(Debug, Default)]
pub(crate) struct ScanTracker {
    observed: HashMap<PathBuf, Observation>,
    accepted: HashMap<PathBuf, Fingerprint>,
}

impl ScanTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan(&mut self, input: &Path) -> AppResult<Vec<StableFile>> {
        let entries = fs::read_dir(input).map_err(|_| AppError::watcher_io())?;
        let mut stable = Vec::new();
        let mut seen = HashSet::new();

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if !file_type.is_file() || !is_supported_image(&path) {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let fingerprint = Fingerprint {
                size: metadata.len(),
                modified: metadata.modified().ok(),
            };

            seen.insert(path.clone());

            if self.accepted.get(&path) == Some(&fingerprint) {
                continue;
            }
            if self.accepted.remove(&path).is_some() {
                self.observed.remove(&path);
            }

            match self.observed.get_mut(&path) {
                Some(observation) if observation.size == fingerprint.size => {
                    observation.count = observation.count.saturating_add(1);
                    if observation.count >= 2 {
                        stable.push(StableFile { path, fingerprint });
                    }
                }
                Some(observation) => {
                    *observation = Observation {
                        size: fingerprint.size,
                        count: 1,
                    };
                }
                None => {
                    self.observed.insert(
                        path,
                        Observation {
                            size: fingerprint.size,
                            count: 1,
                        },
                    );
                }
            }
        }

        self.observed.retain(|path, _| seen.contains(path));
        self.accepted.retain(|path, _| seen.contains(path));
        Ok(stable)
    }

    fn mark_accepted(&mut self, file: &StableFile) {
        self.accepted.insert(file.path.clone(), file.fingerprint);
        self.observed.remove(&file.path);
    }
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let lower = extension.to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::queue::QueueStore;

    use super::{ScanTracker, scan_and_enqueue};

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
    fn stable_file_is_enqueued_exactly_once() {
        let dir = unique_dir("watcher-once");
        let queue_path = dir.join("queue.json");
        let image_path = dir.join("anh.jpg");
        fs::write(&image_path, b"noi-dung-anh").unwrap();

        let mut tracker = ScanTracker::new();
        let mut queue = QueueStore::load(queue_path);

        let first = scan_and_enqueue(&dir, &mut tracker, &mut queue, |_| {}).unwrap();
        let second = scan_and_enqueue(&dir, &mut tracker, &mut queue, |_| {}).unwrap();
        let third = scan_and_enqueue(&dir, &mut tracker, &mut queue, |_| {}).unwrap();

        assert!(first.is_empty());
        assert_eq!(second.len(), 1);
        assert!(third.is_empty());
        assert_eq!(queue.state().unwrap().total, 1);
    }

    #[test]
    fn size_change_resets_stability() {
        let dir = unique_dir("watcher-size");
        let image_path = dir.join("anh.png");
        fs::write(&image_path, b"a").unwrap();

        let mut tracker = ScanTracker::new();
        assert!(tracker.scan(&dir).unwrap().is_empty());
        fs::write(&image_path, b"abcdef").unwrap();
        assert!(tracker.scan(&dir).unwrap().is_empty());
        assert_eq!(tracker.scan(&dir).unwrap().len(), 1);
    }

    #[test]
    fn unsupported_files_are_ignored() {
        let dir = unique_dir("watcher-extension");
        fs::write(dir.join("ghi-chu.txt"), b"noi-dung").unwrap();

        let mut tracker = ScanTracker::new();
        assert!(tracker.scan(&dir).unwrap().is_empty());
        assert!(tracker.scan(&dir).unwrap().is_empty());
    }
}
