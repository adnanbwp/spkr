use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::state::{AppState, RecordingState};

static PTT_PRESS_TIME: Mutex<Option<Instant>> = Mutex::new(None);

/// Register global hotkeys from settings strings.
/// Unregisters all existing shortcuts first, then registers toggle and/or PTT.
/// Shortcuts fire regardless of app focus; handlers check RecordingState internally
/// so they are no-ops when the app is Inactive.
pub fn register_hotkeys(
    app: &AppHandle,
    toggle_app_hotkey: Option<&str>,
    global_hotkey: Option<&str>,
    ptt_hotkey: Option<&str>,
) {
    let _ = app.global_shortcut().unregister_all();

    if let Some(s) = toggle_app_hotkey {
        register_app_toggle(app, s);
    }

    if let Some(s) = global_hotkey {
        if toggle_app_hotkey != Some(s) {
            register_toggle(app, s);
        }
    }

    if let Some(s) = ptt_hotkey {
        if toggle_app_hotkey != Some(s) && global_hotkey != Some(s) {
            register_ptt(app, s);
        }
    }
}

fn register_app_toggle(app: &AppHandle, shortcut: &str) {
    let app_clone = app.clone();
    if let Err(e) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _s, event| {
            if event.state() == ShortcutState::Pressed {
                let state = app_clone.state::<AppState>();
                crate::state::toggle_listening_internal(&app_clone, &state);
            }
        })
    {
        eprintln!("Failed to register app toggle hotkey '{shortcut}': {e}");
    }
}

fn register_toggle(app: &AppHandle, shortcut: &str) {
    let app_clone = app.clone();
    if let Err(e) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _s, event| {
            if event.state() == ShortcutState::Pressed {
                on_toggle_press(&app_clone);
            }
        })
    {
        eprintln!("Failed to register global hotkey '{shortcut}': {e}");
    }
}

fn register_ptt(app: &AppHandle, shortcut: &str) {
    let app_clone = app.clone();
    if let Err(e) = app
        .global_shortcut()
        .on_shortcut(shortcut, move |_app, _s, event| {
            match event.state() {
                ShortcutState::Pressed => on_ptt_press(&app_clone),
                ShortcutState::Released => on_ptt_release(&app_clone),
            }
        })
    {
        eprintln!("Failed to register PTT hotkey '{shortcut}': {e}");
    }
}

fn on_toggle_press(app: &AppHandle) {
    let current = {
        let state = app.state::<AppState>();
        let guard = state.recording_state.lock().unwrap();
        guard.clone()
    };
    match current {
        RecordingState::Listening => {
            if let Err(e) = crate::audio::start_recording_internal(app) {
                eprintln!("Hotkey: start recording error: {e}");
            }
        }
        RecordingState::Recording => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::audio::stop_recording_internal(app).await {
                    eprintln!("Hotkey: stop recording error: {e}");
                }
            });
        }
        _ => {} // Inactive or Transcribing — ignore
    }
}

fn on_ptt_press(app: &AppHandle) {
    *PTT_PRESS_TIME.lock().unwrap() = Some(Instant::now());
    let state = app.state::<AppState>();
    if matches!(
        *state.recording_state.lock().unwrap(),
        RecordingState::Listening
    ) {
        if let Err(e) = crate::audio::start_recording_internal(app) {
            eprintln!("PTT press: start recording error: {e}");
        }
    }
}

fn on_ptt_release(app: &AppHandle) {
    let held_ms = PTT_PRESS_TIME
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0);
    eprintln!("[TIMING] PTT held={}ms", held_ms);

    let current = {
        let state = app.state::<AppState>();
        let guard = state.recording_state.lock().unwrap();
        guard.clone()
    };
    if matches!(current, RecordingState::Recording) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::audio::stop_recording_internal(app).await {
                eprintln!("PTT release: stop recording error: {e}");
            }
        });
    }
}
