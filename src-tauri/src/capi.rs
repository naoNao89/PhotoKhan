//! C-ABI exports for the Windows inference DLL (`photokhan_lib.dll`).
//!
//! Drop-in contract: `phk_process("anh.jpg", "out/")` → writes an ID-photo JPG
//! (preset 3×4, crop/alignment với padding trắng cho vùng ngoài ảnh gốc — chưa có
//! tách nền) vào `out/`.
//!
//! Thư mục model được giải quyết theo thứ tự:
//!   1. `phk_set_models_dir` (đặt từ host)
//!   2. biến môi trường `PHOTOKHAN_MODELS_DIR`
//!   3. `<thư mục DLL>\models` (bundle tự chứa — `current_exe()` là HOST exe nên
//!      phải lấy đường dẫn DLL qua GetModuleHandleExW/GetModuleFileNameW)
//!   4. `<thư mục host exe>\models`
//!
//! Mọi hàm đều không panicked ra ngoài biên giới FFI (catch_unwind).

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::models::ModelStore;
use crate::pipeline::{ProcessOptions, process_single_image};

/// Mã trả về của `phk_process`.
pub const PHK_OK: i32 = 0;
pub const PHK_ERR_ARGS: i32 = 1;
pub const PHK_ERR_MODEL: i32 = 2;
pub const PHK_ERR_INPUT: i32 = 3;
pub const PHK_ERR_PROCESS: i32 = 4;
pub const PHK_ERR_PANIC: i32 = 5;

static MODELS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_last_error(message: impl Into<String>) {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = Some(message.into());
    }
}

fn cstr_to_path(ptr: *const c_char) -> Option<PathBuf> {
    if ptr.is_null() {
        return None;
    }
    let cstr = unsafe { CStr::from_ptr(ptr) };
    let text = cstr.to_str().ok()?;
    Some(PathBuf::from(text))
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleExW(
        flags: u32,
        module_name: *const u16,
        module: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn GetModuleFileNameW(module: *mut core::ffi::c_void, filename: *mut u16, size: u32) -> u32;
}

const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x4;
const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 0x2;

/// Điểm mốc để lấy handle của chính DLL (FROM_ADDRESS).
#[inline(never)]
fn module_marker() {}

/// Thư mục chứa chính DLL (không phải host exe). `current_exe()` trả về host
/// khi DLL được load bởi tiến trình khác, nên lấy qua kernel32.
fn dll_dir() -> Option<PathBuf> {
    unsafe {
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let marker_addr = module_marker as *const () as *const u16;
        if GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            marker_addr,
            &mut handle,
        ) == 0
        {
            return None;
        }
        let mut buffer = [0_u16; 4096];
        let len = GetModuleFileNameW(handle, buffer.as_mut_ptr(), buffer.len() as u32);
        if len == 0 {
            return None;
        }
        let wide = &buffer[..len as usize];
        let path = String::from_utf16(wide).ok()?;
        Path::new(&path).parent().map(Path::to_path_buf)
    }
}

/// Thư mục model: override → env → `<DLL dir>\models` → `<host exe>\models`.
/// Chỉ chấp nhận thư mục có `manifest.json` để `ModelStore` xác minh SHA-256 sau đó.
fn models_dir() -> AppResult<PathBuf> {
    let candidates: Vec<PathBuf> = {
        let mut list = Vec::new();
        if let Ok(guard) = MODELS_DIR.lock() {
            if let Some(dir) = guard.as_ref() {
                list.push(dir.clone());
            }
        }
        if let Some(dir) = std::env::var_os("PHOTOKHAN_MODELS_DIR") {
            list.push(PathBuf::from(dir));
        }
        if let Some(dir) = dll_dir() {
            list.push(dir.join("models"));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                list.push(dir.join("models"));
            }
        }
        list
    };

    for dir in candidates {
        if dir.join(crate::models::MANIFEST_FILE_NAME).is_file() {
            return Ok(dir);
        }
    }
    Err(AppError::model_invalid())
}

/// Đặt thư mục model (chứa `manifest.json` + tệp `.onnx`). Trả về 0 khi ok.
#[unsafe(no_mangle)]
pub extern "C" fn phk_set_models_dir(dir: *const c_char) -> i32 {
    match cstr_to_path(dir) {
        Some(path) => {
            if let Ok(mut slot) = MODELS_DIR.lock() {
                *slot = Some(path);
                PHK_OK
            } else {
                set_last_error("phk_set_models_dir: internal lock failure");
                PHK_ERR_PROCESS
            }
        }
        None => {
            set_last_error("phk_set_models_dir: path argument is null/invalid");
            PHK_ERR_ARGS
        }
    }
}

/// Xử lý một ảnh chân dung: phát hiện đúng một khuôn mặt (E002 nếu không có,
/// E003 nếu nhiều hơn một), làm thẳng, cắt theo preset 3×4 và ghi JPG vào
/// `output_dir`. Trả về [`PHK_OK`] khi thành công; tra cứu chi tiết qua
/// `phk_last_error`.
#[unsafe(no_mangle)]
pub extern "C" fn phk_process(input_path: *const c_char, output_dir: *const c_char) -> i32 {
    catch_unwind(AssertUnwindSafe(|| run_process(input_path, output_dir))).unwrap_or_else(|_| {
        set_last_error("phk_process: panic caught inside DLL");
        PHK_ERR_PANIC
    })
}

fn run_process(input_path: *const c_char, output_dir: *const c_char) -> i32 {
    let source = match cstr_to_path(input_path) {
        Some(path) => path,
        None => {
            set_last_error("phk_process: input path is null/invalid");
            return PHK_ERR_ARGS;
        }
    };
    let out_dir = match cstr_to_path(output_dir) {
        Some(path) => path,
        None => {
            set_last_error("phk_process: output dir is null/invalid");
            return PHK_ERR_ARGS;
        }
    };

    if std::fs::create_dir_all(&out_dir).is_err() {
        set_last_error("phk_process: cannot create output dir");
        return PHK_ERR_PROCESS;
    }

    let models = match models_dir() {
        Ok(dir) => ModelStore::new(dir),
        Err(error) => {
            set_last_error(format!("{}: {}", error.code, error.message_vi));
            return PHK_ERR_MODEL;
        }
    };

    let options = ProcessOptions::default();
    match process_single_image(&source, &out_dir, &models, &options) {
        Ok(report) => {
            set_last_error(String::new());
            tracing::info!(output = %report.output_path.display(), "phk_process OK");
            PHK_OK
        }
        Err(error) => {
            set_last_error(format!("{}: {}", error.code, error.message_vi));
            if error.code == "E004" {
                PHK_ERR_MODEL
            } else if matches!(error.code.as_str(), "E001" | "E002" | "E003") {
                PHK_ERR_INPUT
            } else {
                PHK_ERR_PROCESS
            }
        }
    }
}

/// Chép thông báo lỗi cuối cùng (UTF-8, rỗng nếu lần gọi trước thành công) vào
/// `buffer` (tối đa `capacity` byte, luôn kết thúc NUL). Trả về số byte cần
/// thiết (kể cả NUL) — nếu lớn hơn `capacity`, gọi lại với buffer đủ lớn.
#[unsafe(no_mangle)]
pub extern "C" fn phk_last_error(buffer: *mut c_char, capacity: usize) -> i32 {
    let message = LAST_ERROR
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default();
    let needed = message.len() + 1;
    if buffer.is_null() || capacity == 0 {
        return needed.min(i32::MAX as usize) as i32;
    }
    let copy_len = message.len().min(capacity - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), buffer.cast::<u8>(), copy_len);
        *buffer.add(copy_len) = 0;
    }
    needed.min(i32::MAX as usize) as i32
}
