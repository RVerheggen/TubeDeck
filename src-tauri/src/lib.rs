mod args;
mod binaries;
mod commands;
mod engine;
mod jsonfile;
mod options;
mod presets;
mod settings;

use commands::{AppState, LOG_FILE_NAME};
use engine::Engine;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be registered first: a second launch focuses the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: Some(LOG_FILE_NAME.into()) }),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(2 * 1024 * 1024)
                .rotation_strategy(RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let paths = app.path();
            let config_dir = paths.app_config_dir()?;
            let log_dir = paths.app_log_dir()?;
            let local_data = paths.app_local_data_dir()?;
            let managed_bin_dir = local_data.join("bin");
            let temp_root = local_data.join("tmp");
            let default_download_folder = paths
                .download_dir()
                .map(|d| d.join("TubeDeck").display().to_string())
                .unwrap_or_default();

            log::info!("TubeDeck {} starting; config in {}", app.package_info().version, config_dir.display());
            app.manage(AppState::load(config_dir, log_dir, managed_bin_dir, default_download_folder));
            app.manage(Engine::new(app.handle().clone(), temp_root));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::build_command,
            commands::parse_urls,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            commands::engine_status,
            commands::read_app_log,
            commands::app_paths,
            commands::enqueue_jobs,
            commands::list_jobs,
            commands::cancel_job,
            commands::retry_job,
            commands::remove_job,
            commands::clear_finished_jobs,
            commands::get_job_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building TubeDeck");

    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Job Objects also kill children when TubeDeck exits; this just does it promptly.
            if let Some(engine) = handle.try_state::<Arc<Engine>>() {
                engine.shutdown();
            }
        }
    });
}
