use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::settings::{AppSettings};

// RecordingState enum — drives overlay color and hotkey behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingState {
    Inactive,      // overlay hidden, hotkeys unregistered
    Listening,     // green, hotkeys active
    Recording,     // pulsing red, mic open
    Transcribing,  // orange, inference in progress
}

impl RecordingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RecordingState::Inactive => "Inactive",
            RecordingState::Listening => "Listening",
            RecordingState::Recording => "Recording",
            RecordingState::Transcribing => "Transcribing",
        }
    }
}

// AppState — Tauri-managed shared state
pub struct AppState {
    pub recording_state: Mutex<RecordingState>,
    pub settings: Mutex<AppSettings>,
    pub audio_buffer: Mutex<Vec<f32>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            recording_state: Mutex::new(RecordingState::Inactive),
            settings: Mutex::new(AppSettings::default()),
            audio_buffer: Mutex::new(Vec::new()),
        }
    }
}

// Internal helper: transition to a new state and emit event to all windows
pub fn set_state(app: &tauri::AppHandle, state: &AppState, new_state: RecordingState) {
    let state_str = new_state.as_str().to_string();
    {
        let mut recording_state = state.recording_state.lock().unwrap();
        *recording_state = new_state;
    }
    let _ = app.emit("state-changed", serde_json::json!({ "state": state_str }));
}

// Returns current state as a string ("Inactive", "Listening", etc.)
#[tauri::command]
pub fn get_recording_state(state: tauri::State<AppState>) -> String {
    let recording_state = state.recording_state.lock().unwrap();
    recording_state.as_str().to_string()
}

// Transitions to Listening if currently Inactive, or to Inactive if Listening
// Emits a "state-changed" event to all windows
#[tauri::command]
pub fn toggle_listening(app: tauri::AppHandle, state: tauri::State<AppState>) {
    let current = {
        let recording_state = state.recording_state.lock().unwrap();
        recording_state.as_str().to_string()
    };
    let new_state = match current.as_str() {
        "Inactive" => RecordingState::Listening,
        _ => RecordingState::Inactive,
    };
    set_state(&app, &state, new_state);
}
