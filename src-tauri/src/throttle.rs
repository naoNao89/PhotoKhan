use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// Số luồng xử lý theo SRS 3.3: `max(1, floor(số nhân × 0.20))`.
pub fn worker_count_for_cores(cores: usize) -> usize {
    (cores / 5).max(1)
}

pub fn worker_count() -> AppResult<usize> {
    let cores = std::thread::available_parallelism()
        .map_err(|_| AppError::worker_io())?
        .get();
    Ok(worker_count_for_cores(cores))
}

/// Đặt luồng hiện tại ở mức BELOW_NORMAL. Dùng FFI trực tiếp để không thêm crate.
pub fn set_current_thread_below_normal() -> AppResult<()> {
    platform::set_current_thread_below_normal()
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_void;

    use crate::error::{AppError, AppResult};

    const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }

    pub fn set_current_thread_below_normal() -> AppResult<()> {
        // SAFETY: GetCurrentThread trả về pseudo-handle của luồng hiện tại và
        // không cần đóng. SetThreadPriority chỉ đọc/ghi thuộc tính của luồng đó.
        let result = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) };

        if result == 0 {
            Err(AppError::worker_io())
        } else {
            Ok(())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use crate::error::AppResult;

    pub fn set_current_thread_below_normal() -> AppResult<()> {
        Ok(())
    }
}

/// Nhóm luồng nền M1. Chưa nhận job; chỉ giữ đúng số luồng và mức ưu tiên
/// để milestone sau gắn hàng đợi xử lý vào.
pub struct WorkerPool {
    shared: Arc<(Mutex<bool>, Condvar)>,
    handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    pub fn start(count: usize) -> AppResult<Self> {
        let shared = Arc::new((Mutex::new(false), Condvar::new()));
        let (init_tx, init_rx) = std::sync::mpsc::channel::<AppResult<()>>();
        let mut handles = Vec::with_capacity(count);
        let mut spawn_error = None;

        for index in 0..count {
            let thread_shared = Arc::clone(&shared);
            let thread_init = init_tx.clone();
            let builder = thread::Builder::new().name(format!("photokhan-worker-{index}"));
            match builder.spawn(move || worker_main(thread_shared, thread_init)) {
                Ok(handle) => handles.push(handle),
                Err(_) => {
                    spawn_error = Some(AppError::worker_io());
                    break;
                }
            }
        }
        drop(init_tx);

        let mut init_error = None;
        for _ in 0..handles.len() {
            match init_rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if init_error.is_none() {
                        init_error = Some(error);
                    }
                }
                Err(_) => {
                    if init_error.is_none() {
                        init_error = Some(AppError::worker_io());
                    }
                }
            }
        }

        if let Some(error) = spawn_error.or(init_error) {
            let _ = stop_workers(&shared, &mut handles);
            return Err(error);
        }

        Ok(Self { shared, handles })
    }

    pub fn stop(&mut self) -> AppResult<()> {
        stop_workers(&self.shared, &mut self.handles)
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            tracing::warn!(
                code = %error.code,
                "Không dừng sạch được luồng xử lý nền: {}",
                error.message_vi
            );
        }
    }
}

fn worker_main(shared: Arc<(Mutex<bool>, Condvar)>, init: std::sync::mpsc::Sender<AppResult<()>>) {
    let init_result = set_current_thread_below_normal();
    let can_continue = init_result.is_ok();
    if init.send(init_result).is_err() || !can_continue {
        return;
    }

    let (lock, condition) = &*shared;
    let mut stopped = match lock.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };

    while !*stopped {
        match condition.wait_timeout(stopped, Duration::from_secs(1)) {
            Ok((guard, _)) => stopped = guard,
            Err(_) => return,
        }
    }
}

fn stop_workers(
    shared: &Arc<(Mutex<bool>, Condvar)>,
    handles: &mut Vec<JoinHandle<()>>,
) -> AppResult<()> {
    let (lock, condition) = &**shared;
    let mut stop_error = None;
    match lock.lock() {
        Ok(mut stopped) => {
            *stopped = true;
            condition.notify_all();
        }
        Err(_) => stop_error = Some(AppError::worker_io()),
    }

    let mut join_error = false;
    for handle in handles.drain(..) {
        if handle.join().is_err() {
            join_error = true;
        }
    }

    if let Some(error) = stop_error {
        Err(error)
    } else if join_error {
        Err(AppError::worker_io())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::worker_count_for_cores;

    #[test]
    fn worker_count_matches_srs_examples() {
        assert_eq!(worker_count_for_cores(1), 1);
        assert_eq!(worker_count_for_cores(4), 1);
        assert_eq!(worker_count_for_cores(8), 1);
        assert_eq!(worker_count_for_cores(16), 3);
        assert_eq!(worker_count_for_cores(32), 6);
    }
}
