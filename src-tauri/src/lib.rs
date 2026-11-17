mod clock;
mod commands;
mod error;
mod export;
mod model;
mod models;
mod persistence;
mod pipeline;
mod queue;
pub mod selftest;
mod settings;
mod state;
mod throttle;
mod watcher;

use tauri::Manager;
use tracing::Level;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .try_init();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState::initialize(&handle)?;
            state.start_workers()?;
            app.manage(state);

            let managed = app.state::<AppState>();
            if let Err(error) = managed.restart_watcher(&handle) {
                tracing::warn!(
                    code = %error.code,
                    "Không khởi động được watcher: {}",
                    error.message_vi
                );
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::choose_input_folder,
            commands::choose_output_folder,
            commands::start_queue,
            commands::pause_queue,
            commands::get_queue_state,
            commands::get_settings,
            commands::set_settings,
            commands::confirm_minor_consent,
            commands::export_batch,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!("Ứng dụng không khởi động được: {error}");
        std::process::exit(1);
    }
}
