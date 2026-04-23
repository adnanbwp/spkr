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
    // Sent to frontend for display in settings UI; store securely if needed in future
    pub groq_api_key: String,
    pub local_model: LocalModel,
    pub input_device: Option<String>,
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
            global_hotkey: None,
            ptt_hotkey: None,
        }
    }
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|e| e.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
pub fn save_settings(state: State<AppState>, settings: AppSettings) -> Result<(), String> {
    let mut current = state.settings.lock().map_err(|e| e.to_string())?;
    *current = settings;
    Ok(())
}
