mod args;
mod binaries;
mod commands;
mod db;
mod diagnostics;
mod engine;
mod installer;
mod jsonfile;
mod options;
mod presets;
mod settings;

use commands::{AppState, LOG_FILE_NAME};
use db::Db;
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
                    Target::new(TargetKind::LogDir {
                        file_name: Some(LOG_FILE_NAME.into()),
                    }),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(2 * 1024 * 1024)
                .rotation_strategy(RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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

            log::info!(
                "TubeDeck {} starting; data in {}",
                app.package_info().version,
                local_data.display()
            );
            let db_path = local_data.join("tubedeck.db");
            let db = match Db::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    // Keep the app usable; nothing will be saved this session.
                    log::error!(
                        "{e} ({}). Using a temporary in-memory database.",
                        db_path.display()
                    );
                    Db::open_in_memory()?
                }
            };
            let db = Arc::new(db);
            let state = AppState::load(
                db.clone(),
                config_dir,
                log_dir.clone(),
                managed_bin_dir,
                default_download_folder,
            );
            let resume = state.settings.lock().unwrap().resume_on_startup;
            app.manage(state);
            app.manage(engine::probe::Probes::default());
            app.manage(commands::Installs::default());
            let engine = Engine::new(
                app.handle().clone(),
                temp_root,
                log_dir.join("jobs"),
                db,
                resume,
            );
            app.manage(engine.clone());
            engine.schedule();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { commands::startup_maintenance(handle).await });
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
            commands::pause_job,
            commands::resume_job,
            commands::pause_all_jobs,
            commands::resume_all_jobs,
            commands::move_job,
            commands::retry_job,
            commands::remove_job,
            commands::clear_finished_jobs,
            commands::get_job_log,
            commands::list_history,
            commands::delete_history,
            commands::clear_history,
            commands::redownload,
            commands::export_diagnostics,
            commands::probe_url,
            commands::cancel_probe,
            commands::enqueue_playlist,
            commands::install_binary,
            commands::check_updates,
            commands::prepare_for_update,
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
