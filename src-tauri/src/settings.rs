use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// TranscriptionBackend enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionBackend {
    Local,
    Groq,
}

// LocalModel enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalModel {
    Base,
    Small,
    Medium,
}

// AppSettings — all user-configurable settings, serializable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub backend: TranscriptionBackend,
    pub groq_api_key: String,
    pub local_model: LocalModel,
    pub input_device: Option<String>,
    pub listening_enabled: bool,
    pub global_hotkey: Option<String>,
    pub ptt_hotkey: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            backend: TranscriptionBackend::Local,
            groq_api_key: String::new(),
            local_model: LocalModel::Base,
            input_device: None,
            listening_enabled: false,
            global_hotkey: None,
            ptt_hotkey: None,
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppSettings {
    let settings = state.settings.lock().unwrap();
    settings.clone()
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: AppSettings) {
    let mut current = state.settings.lock().unwrap();
    *current = settings;
}
