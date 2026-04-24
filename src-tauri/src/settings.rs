use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_store::StoreExt;

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
    pub toggle_app_hotkey: Option<String>,
    pub global_hotkey: Option<String>,
    pub ptt_hotkey: Option<String>,
}

impl LocalModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LocalModel::Base => "Base",
            LocalModel::Small => "Small",
            LocalModel::Medium => "Medium",
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            backend: TranscriptionBackend::Local,
            groq_api_key: String::new(),
            local_model: LocalModel::Base,
            input_device: None,
            toggle_app_hotkey: None,
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
pub fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    settings: AppSettings,
) -> Result<(), String> {
    {
        let mut current = state.settings.lock().map_err(|e| e.to_string())?;
        *current = settings.clone();
    }
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("settings", serde_json::to_value(&settings).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;

    crate::hotkeys::register_hotkeys(
        &app,
        settings.toggle_app_hotkey.as_deref(),
        settings.global_hotkey.as_deref(),
        settings.ptt_hotkey.as_deref(),
    );

    // Invalidate model cache — backend or model may have changed
    {
        let mut cached = state.cached_model.lock().unwrap();
        *cached = None;
    }
    // Spawn background re-load for the new settings
    if matches!(settings.backend, TranscriptionBackend::Local) {
        if let Ok(path) = crate::models::model_path(&app, &settings.local_model) {
            if path.exists() {
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    crate::transcription::warm_model_cache(&handle, path);
                });
            }
        }
    }

    Ok(())
}

pub fn load_settings_from_store(app: &tauri::AppHandle, state: &AppState) {
    let Ok(store) = app.store("settings.json") else { return };
    let Some(val) = store.get("settings") else { return };
    let Ok(loaded) = serde_json::from_value::<AppSettings>(val) else { return };
    if let Ok(mut s) = state.settings.lock() {
        *s = loaded;
    }
}
