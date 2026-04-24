use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::settings::{LocalModel, TranscriptionBackend};

#[derive(Debug, Clone, serde::Serialize)]
pub struct TranscriptionTiming {
    pub model_load_ms: u64,
    pub inference_ms: u64,
    pub segment_collect_ms: u64,
    pub total_transcription_ms: u64,
}

pub struct CachedWhisperModel {
    pub context: Arc<whisper_rs::WhisperContext>,
    pub model_path: PathBuf,
}

// Safety: WhisperContext wraps a C pointer; we only access it from spawn_blocking
// (one blocking thread at a time, serialized by the Mutex in AppState).
unsafe impl Send for CachedWhisperModel {}
unsafe impl Sync for CachedWhisperModel {}

pub fn warm_model_cache(app: &tauri::AppHandle, path: PathBuf) {
    use tauri::Manager;
    use whisper_rs::{WhisperContext, WhisperContextParameters};

    eprintln!("[CACHE] Loading model into cache: {}", path.display());
    match WhisperContext::new_with_params(
        path.to_str().unwrap_or(""),
        WhisperContextParameters::default(),
    ) {
        Ok(ctx) => {
            let state = app.state::<crate::state::AppState>();
            let mut cached = state.cached_model.lock().unwrap();
            *cached = Some(CachedWhisperModel {
                context: Arc::new(ctx),
                model_path: path,
            });
            eprintln!("[CACHE] Model cached successfully");
        }
        Err(e) => eprintln!("[CACHE] Failed to pre-load model: {e}"),
    }
}

pub async fn transcribe(
    audio: Vec<f32>,
    backend: TranscriptionBackend,
    local_model: LocalModel,
    groq_api_key: String,
    app: &tauri::AppHandle,
) -> Result<(String, Option<TranscriptionTiming>), String> {
    match backend {
        TranscriptionBackend::Local => {
            let path = crate::models::model_path(app, &local_model)?;
            // Try to use cached context (cache hit = near-zero model_load_ms)
            let cached_ctx = {
                use tauri::Manager;
                let state = app.state::<crate::state::AppState>();
                let guard = state.cached_model.lock().unwrap();
                guard
                    .as_ref()
                    .filter(|m| m.model_path == path)
                    .map(|m| m.context.clone())
            };
            let (text, timing) =
                tokio::task::spawn_blocking(move || run_local(audio, path, cached_ctx))
                    .await
                    .map_err(|e| format!("Thread error: {e}"))??;
            Ok((text, Some(timing)))
        }
        TranscriptionBackend::Groq => {
            let text = run_groq(audio, &groq_api_key).await?;
            Ok((text, None))
        }
    }
}

fn run_local(
    audio: Vec<f32>,
    model_path: PathBuf,
    cached_ctx: Option<Arc<whisper_rs::WhisperContext>>,
) -> Result<(String, TranscriptionTiming), String> {
    use whisper_rs::{FullParams, SamplingStrategy};

    let t0 = Instant::now();

    let ctx = if let Some(c) = cached_ctx {
        eprintln!("[CACHE] Cache hit — skipping model load");
        c
    } else {
        use whisper_rs::{WhisperContext, WhisperContextParameters};
        if !model_path.exists() {
            return Err(format!("Model not downloaded: {}", model_path.display()));
        }
        eprintln!("[CACHE] Cache miss — loading model from disk");
        Arc::new(
            WhisperContext::new_with_params(
                model_path.to_str().ok_or("Invalid model path")?,
                WhisperContextParameters::default(),
            )
            .map_err(|e| format!("Failed to load model: {e}"))?,
        )
    };

    let model_load_ms = t0.elapsed().as_millis() as u64;
    eprintln!("[TIMING] model_load={}ms", model_load_ms);

    let mut state = ctx.create_state().map_err(|e| format!("Whisper state error: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    let t_infer = Instant::now();
    state
        .full(params, &audio)
        .map_err(|e| format!("Transcription failed: {e}"))?;
    let inference_ms = t_infer.elapsed().as_millis() as u64;
    eprintln!("[TIMING] inference={}ms", inference_ms);

    let t_seg = Instant::now();
    let n = state.full_n_segments();
    let mut text = String::new();
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            text.push_str(seg.to_str().map_err(|e| format!("Segment error: {e}"))?);
        }
    }
    let segment_collect_ms = t_seg.elapsed().as_millis() as u64;
    let total_transcription_ms = t0.elapsed().as_millis() as u64;
    eprintln!("[TIMING] total_transcription={}ms", total_transcription_ms);

    Ok((
        text.trim().to_string(),
        TranscriptionTiming {
            model_load_ms,
            inference_ms,
            segment_collect_ms,
            total_transcription_ms,
        },
    ))
}

async fn run_groq(audio: Vec<f32>, api_key: &str) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("Groq API key not set".to_string());
    }

    let wav_bytes = pcm_to_wav(&audio, 16000);

    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-large-v3-turbo")
        .text("response_format", "json");

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Groq API error {status}: {body}"));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    json["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "Unexpected Groq response format".to_string())
}

fn pcm_to_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (samples.len() * 2) as u32;

    let mut buf = Vec::with_capacity(44 + data_size as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_size).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        let i = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        buf.extend_from_slice(&i.to_le_bytes());
    }
    buf
}
