use std::fs;
use std::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::model::Settings;
use crate::queue::QueueStore;
use crate::settings::SettingsStore;
use crate::throttle::WorkerPool;
use crate::watcher::WatcherManager;

/// Trạng thái dùng chung của ứng dụng. Các khóa Mutex chỉ giữ trong thời gian
/// đọc/ghi ngắn, không giữ qua lời gọi mạng hay hộp thoại.
pub struct AppState {
    pub queue: Mutex<QueueStore>,
    pub settings: Mutex<SettingsStore>,
    pub watcher: Mutex<WatcherManager>,
    pub workers: Mutex<Option<WorkerPool>>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| AppError::settings_io())?;
        fs::create_dir_all(&data_dir).map_err(|_| AppError::settings_io())?;

        let queue_path = data_dir.join("queue.json");
        let settings_path = data_dir.join("settings.json");

        Ok(Self {
            queue: Mutex::new(QueueStore::load(queue_path)),
            settings: Mutex::new(SettingsStore::load(settings_path)),
            watcher: Mutex::new(WatcherManager::new()),
            workers: Mutex::new(None),
        })
    }

    pub fn start_workers(&self) -> AppResult<()> {
        let count = crate::throttle::worker_count()?;
        let pool = WorkerPool::start(count)?;
        let mut workers = self.workers.lock().map_err(|_| AppError::worker_io())?;
        *workers = Some(pool);
        Ok(())
    }

    pub fn lock_queue(&self) -> AppResult<MutexGuard<'_, QueueStore>> {
        self.queue.lock().map_err(|_| AppError::queue_io())
    }

    pub fn lock_settings(&self) -> AppResult<MutexGuard<'_, SettingsStore>> {
        self.settings.lock().map_err(|_| AppError::settings_io())
    }

    pub fn lock_watcher(&self) -> AppResult<MutexGuard<'_, WatcherManager>> {
        self.watcher.lock().map_err(|_| AppError::watcher_io())
    }

    pub fn settings_snapshot(&self) -> AppResult<Settings> {
        self.lock_settings()?.get()
    }

    pub fn restart_watcher(&self, app: &AppHandle) -> AppResult<()> {
        let settings = self.settings_snapshot()?;
        self.lock_watcher()?.restart(app.clone(), &settings)
    }
}
