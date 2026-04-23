use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;
use tauri::{Emitter, Manager};

use crate::settings::LocalModel;

const BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

fn model_filename(model: &LocalModel) -> &'static str {
    match model {
        LocalModel::Base => "ggml-base.en.bin",
        LocalModel::Small => "ggml-small.en.bin",
        LocalModel::Medium => "ggml-medium.en.bin",
    }
}

fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn model_path(app: &tauri::AppHandle, model: &LocalModel) -> Result<PathBuf, String> {
    Ok(models_dir(app)?.join(model_filename(model)))
}

#[derive(Serialize)]
pub struct ModelInfo {
    pub model: String,
    pub filename: String,
    pub downloaded: bool,
    pub size_bytes: Option<u64>,
}

#[tauri::command]
pub fn list_models(app: tauri::AppHandle) -> Result<Vec<ModelInfo>, String> {
    let dir = models_dir(&app)?;
    [LocalModel::Base, LocalModel::Small, LocalModel::Medium]
        .iter()
        .map(|m| {
            let filename = model_filename(m).to_string();
            let path = dir.join(&filename);
            let downloaded = path.exists();
            let size_bytes = downloaded
                .then(|| fs::metadata(&path).ok()?.len().into())
                .flatten();
            Ok(ModelInfo {
                model: m.as_str().to_string(),
                filename,
                downloaded,
                size_bytes,
            })
        })
        .collect()
}

async fn write_download(
    app: &tauri::AppHandle,
    mut response: reqwest::Response,
    total: u64,
    tmp: &PathBuf,
    dest: &PathBuf,
    model_name: &str,
) -> Result<(), String> {
    let mut file = fs::File::create(tmp).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        let fraction = if total > 0 { downloaded as f64 / total as f64 } else { 0.0 };
        let _ = app.emit(
            "download-progress",
            serde_json::json!({
                "model": model_name,
                "downloaded": downloaded,
                "total": total,
                "fraction": fraction,
            }),
        );
    }

    fs::rename(tmp, dest).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_model(
    app: tauri::AppHandle,
    model: LocalModel,
) -> Result<(), String> {
    let model_name = model.as_str();
    let url = format!("{BASE_URL}/{}", model_filename(&model));
    let dest = model_path(&app, &model)?;
    let tmp = dest.with_extension("bin.tmp");

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);
    let result = write_download(&app, response, total, &tmp, &dest, model_name).await;

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    } else {
        let _ = app.emit("download-complete", serde_json::json!({ "model": model_name }));
    }

    result
}

#[tauri::command]
pub fn delete_model(app: tauri::AppHandle, model: LocalModel) -> Result<(), String> {
    let path = model_path(&app, &model)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
