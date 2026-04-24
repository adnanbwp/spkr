mod state;
mod settings;
mod audio;
mod transcription;
mod models;
mod hotkeys;
mod injector;

use state::AppState;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            state::get_recording_state,
            state::toggle_listening,
            settings::get_settings,
            settings::save_settings,
            audio::list_input_devices,
            audio::start_recording,
            audio::stop_recording,
            models::list_models,
            models::download_model,
            models::delete_model,
        ])
        .setup(|app| {
            // Load persisted settings before anything else uses them
            let app_state = app.state::<AppState>();
            settings::load_settings_from_store(app.handle(), &app_state);

            // Register hotkeys from loaded settings (if any were saved previously)
            {
                let s = app_state.settings.lock().unwrap();
                hotkeys::register_hotkeys(
                    app.handle(),
                    s.toggle_app_hotkey.as_deref(),
                    s.global_hotkey.as_deref(),
                    s.ptt_hotkey.as_deref(),
                );
            }

            // Pre-load local Whisper model into cache at startup
            {
                let (backend, local_model) = {
                    let s = app_state.settings.lock().unwrap();
                    (s.backend.clone(), s.local_model.clone())
                };
                if matches!(backend, settings::TranscriptionBackend::Local) {
                    if let Ok(path) = models::model_path(app.handle(), &local_model) {
                        if path.exists() {
                            let handle = app.handle().clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                transcription::warm_model_cache(&handle, path);
                            });
                        }
                    }
                }
            }

            // Register app to run at login
            {
                use tauri_plugin_autostart::ManagerExt;
                if let Err(e) = app.autolaunch().enable() {
                    eprintln!("Failed to enable autostart: {e}");
                }
            }

            // Hide settings window on close instead of destroying it so it can be reopened
            if let Some(settings_win) = app.get_webview_window("settings") {
                let win = settings_win.clone();
                settings_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win.hide();
                    }
                });
            }

            // Click-through overlay, positioned at top-right of primary monitor with 16px margin
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.set_ignore_cursor_events(true);

                if let Ok(Some(monitor)) = overlay.primary_monitor() {
                    let scale = monitor.scale_factor();
                    let size = monitor.size();
                    let logical_w = size.width as f64 / scale;
                    let x = logical_w - 80.0 - 16.0;
                    let _ = overlay.set_position(tauri::LogicalPosition::new(x, 16.0));
                }
            }

            // System tray
            let open_item = MenuItem::with_id(app, "open_settings", "Open Settings", true, None::<&str>)?;
            let toggle_item = MenuItem::with_id(app, "toggle_listening", "Toggle Listening", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_item, &toggle_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(tauri::include_image!("icons/32x32.png"))
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open_settings" => {
                        if let Some(win) = app.get_webview_window("settings") {
                            let _ = win.set_size(tauri::LogicalSize::new(600.0_f64, 620.0_f64));
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "toggle_listening" => {
                        let s = app.state::<AppState>();
                        state::toggle_listening_internal(app, &s);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
