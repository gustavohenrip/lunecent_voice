mod audio;
mod cleanup;
mod commands;
mod config;
mod dictionary;
mod error;
mod history;
mod hotkey;
#[cfg(windows)]
mod inputhook;
mod inject;
mod models;
mod pipeline;
mod services;
mod sidecar;
mod state;
mod transcribe;
mod tray;
mod vad;

use crate::audio::AudioEngine;
use crate::cleanup::LlmClient;
use crate::config::Settings;
use crate::state::{AppState, EngineMeta, SharedState, Status};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_window(app, "widget");
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .setup(|app| {
            if let Err(err) = setup(app) {
                tracing::error!("setup failed: {err}");
                return Err(err);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::save_settings,
            commands::list_audio_devices,
            commands::toggle_recording,
            commands::cancel_recording,
            commands::get_history,
            commands::search_history,
            commands::delete_history,
            commands::clear_history,
            commands::get_stats,
            commands::recopy,
            commands::add_to_dictionary,
            commands::model_statuses,
            commands::download_model,
            commands::reload_engine,
            commands::restart_llm,
            commands::test_llm,
            commands::set_autostart,
            commands::open_window,
            commands::hide_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running Lunecent Voice");
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();

    let config_dir = handle.path().app_config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    let data_dir = handle.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let models_dir = data_dir.join("models");
    std::fs::create_dir_all(&models_dir)?;
    let resource_dir = handle
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let config_path = config_dir.join("settings.json");

    let settings = Settings::load(&config_path);
    let _ = settings.save(&config_path);

    prepend_dll_dirs(&resource_dir);

    let audio = AudioEngine::new(settings.audio_device.clone());

    let db_path = data_dir.join("history.db");
    let connection = history::open(&db_path)?;

    let state: SharedState = Arc::new(AppState {
        app: handle.clone(),
        config_path,
        models_dir,
        resource_dir,
        settings: RwLock::new(settings),
        audio,
        transcribe: RwLock::new(None),
        engine_meta: RwLock::new(EngineMeta::default()),
        vad: RwLock::new(None),
        llm: LlmClient::new(),
        sidecar: Mutex::new(None),
        db: Mutex::new(connection),
        status: RwLock::new(Status::Idle),
        recording: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        sidecar_ready: AtomicBool::new(false),
        downloading: Mutex::new(std::collections::HashSet::new()),
    });

    app.manage(state.clone());

    #[cfg(windows)]
    inputhook::start(handle.clone());

    tray::build(&handle)?;
    position_widget(&handle);

    #[cfg(windows)]
    round_window_corners(&handle);

    spawn_level_emitter(state.clone());

    services::bootstrap(&handle, &state);

    Ok(())
}

fn spawn_level_emitter(state: SharedState) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if state.recording.load(Ordering::Acquire) {
                let _ = state.app.emit("audio-level", state.audio.level());
            }
        }
    });
}

#[cfg(windows)]
fn round_window_corners(app: &tauri::AppHandle) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    for label in ["settings", "history"] {
        if let Some(window) = app.get_webview_window(label) {
            if let Ok(hwnd) = window.hwnd() {
                let preference = DWMWCP_ROUND;
                unsafe {
                    let _ = DwmSetWindowAttribute(
                        windows::Win32::Foundation::HWND(hwnd.0 as *mut core::ffi::c_void),
                        DWMWA_WINDOW_CORNER_PREFERENCE,
                        &preference as *const _ as *const core::ffi::c_void,
                        std::mem::size_of_val(&preference) as u32,
                    );
                }
            }
        }
    }
}

fn show_window(app: &tauri::AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn position_widget(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("widget") {
        if let Ok(Some(monitor)) = window.current_monitor() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            let scale = monitor.scale_factor();
            let widget_size = window.outer_size().unwrap_or(tauri::PhysicalSize {
                width: (268.0 * scale) as u32,
                height: (40.0 * scale) as u32,
            });
            let margin = (24.0 * scale) as i32;
            let taskbar = (64.0 * scale) as i32;
            let x = monitor_pos.x + monitor_size.width as i32 - widget_size.width as i32 - margin;
            let y =
                monitor_pos.y + monitor_size.height as i32 - widget_size.height as i32 - taskbar;
            let _ = window.set_position(tauri::PhysicalPosition { x, y });
        }
    }
}

fn prepend_dll_dirs(resource_dir: &Path) {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        resource_dir.to_path_buf(),
        resource_dir.join("resources"),
        resource_dir.join("resources").join("cuda"),
        resource_dir.join("resources").join("binaries"),
        manifest.join("resources"),
        manifest.join("resources").join("cuda"),
        manifest.join("resources").join("binaries"),
    ];
    let existing: Vec<PathBuf> = candidates.into_iter().filter(|p| p.exists()).collect();
    if existing.is_empty() {
        return;
    }
    let prefix = existing
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(";");
    let current = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{prefix};{current}"));
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
