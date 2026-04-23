use std::path::PathBuf;

use crate::settings::{LocalModel, TranscriptionBackend};

pub async fn transcribe(
    audio: Vec<f32>,
    backend: TranscriptionBackend,
    local_model: LocalModel,
    groq_api_key: String,
    app: &tauri::AppHandle,
) -> Result<String, String> {
    match backend {
        TranscriptionBackend::Local => {
            let path = crate::models::model_path(app, &local_model)?;
            tokio::task::spawn_blocking(move || run_local(audio, path))
                .await
                .map_err(|e| format!("Thread error: {e}"))?
        }
        TranscriptionBackend::Groq => run_groq(audio, &groq_api_key).await,
    }
}

fn run_local(audio: Vec<f32>, model_path: PathBuf) -> Result<String, String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    if !model_path.exists() {
        return Err(format!("Model not downloaded: {}", model_path.display()));
    }

    let ctx = WhisperContext::new_with_params(
        model_path.to_str().ok_or("Invalid model path")?,
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load model: {e}"))?;

    let mut state = ctx.create_state().map_err(|e| format!("Whisper state error: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state
        .full(params, &audio)
        .map_err(|e| format!("Transcription failed: {e}"))?;

    let n = state.full_n_segments();
    let mut text = String::new();
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            text.push_str(seg.to_str().map_err(|e| format!("Segment error: {e}"))?);
        }
    }

    Ok(text.trim().to_string())
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

// Encode f32 PCM samples as a 16-bit PCM WAV in memory (no hound needed).
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
