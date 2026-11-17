use std::path::Path;

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, AppResult};
use crate::model::{QueueState, Settings};
use crate::state::AppState;

#[tauri::command]
pub async fn choose_input_folder(app: AppHandle) -> AppResult<String> {
    let picked = pick_folder(app, "Chọn thư mục ảnh gốc").await?;
    Ok(picked)
}

#[tauri::command]
pub async fn choose_output_folder(app: AppHandle) -> AppResult<String> {
    let picked = pick_folder(app, "Chọn thư mục lưu ảnh").await?;
    Ok(picked)
}

async fn pick_folder(app: AppHandle, title: &'static str) -> AppResult<String> {
    let dialog_app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .set_title(title)
            .blocking_pick_folder()
    })
    .await
    .map_err(|_| AppError::dialog_error())?;

    Ok(picked.map(|path| path.to_string()).unwrap_or_default())
}

#[tauri::command]
pub fn start_queue(state: State<'_, AppState>) -> AppResult<()> {
    state.lock_queue()?.set_paused(false)
}

#[tauri::command]
pub fn pause_queue(state: State<'_, AppState>) -> AppResult<()> {
    state.lock_queue()?.set_paused(true)
}

#[tauri::command]
pub fn get_queue_state(state: State<'_, AppState>) -> AppResult<QueueState> {
    state.lock_queue()?.state()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.settings_snapshot()
}

#[tauri::command]
pub fn set_settings(
    settings: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<()> {
    validate_input_dir(&settings)?;
    state.lock_settings()?.set(settings)?;
    state.restart_watcher(&app)
}

#[tauri::command]
pub fn confirm_minor_consent(version: String, state: State<'_, AppState>) -> AppResult<()> {
    state.lock_settings()?.set_consent(&version)
}

#[tauri::command]
pub fn export_batch(preset: String, bg_hex: String, outfit: Option<String>) -> AppResult<()> {
    // M1 chỉ giữ chữ ký theo CONTRACT; pipeline xuất ảnh thuộc milestone sau.
    let _ = (preset, bg_hex, outfit);
    Err(AppError::not_ready())
}

fn validate_input_dir(settings: &Settings) -> AppResult<()> {
    let Some(input_dir) = settings
        .input_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    if Path::new(input_dir).is_dir() {
        Ok(())
    } else {
        Err(AppError::input_dir_invalid())
    }
}
