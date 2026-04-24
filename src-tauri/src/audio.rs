use std::sync::Mutex;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{Emitter, Manager};

use crate::state::{AppState, RecordingState, set_state};

#[allow(dead_code)]
struct StreamHandle(cpal::Stream);
unsafe impl Send for StreamHandle {}
unsafe impl Sync for StreamHandle {}

static ACTIVE_STREAM: Mutex<Option<StreamHandle>> = Mutex::new(None);
static CAPTURE_RATE: Mutex<u32> = Mutex::new(16000);
static CAPTURE_CHANNELS: Mutex<u16> = Mutex::new(1);
static PTT_RELEASE_TIME: Mutex<Option<Instant>> = Mutex::new(None);

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    Ok(devices.filter_map(|d| d.name().ok()).collect())
}

pub fn start_recording_internal(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let host = cpal::default_host();

    let device_name = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        settings.input_device.clone()
    };

    let device = if let Some(ref name) = device_name {
        let found = host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().ok().as_deref() == Some(name.as_str()));
        match found {
            Some(d) => d,
            None => {
                eprintln!("Input device '{name}' not found, falling back to system default");
                host.default_input_device()
                    .ok_or_else(|| "No default input device available".to_string())?
            }
        }
    } else {
        host.default_input_device()
            .ok_or_else(|| "No default input device available".to_string())?
    };

    let audio_buffer = state.audio_buffer.clone();
    {
        let mut buf = audio_buffer.lock().map_err(|e| e.to_string())?;
        buf.clear();
    }

    // Try 16 kHz mono (ideal for Whisper); fall back to device native config.
    let preferred = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16000),
        buffer_size: cpal::BufferSize::Default,
    };

    let buf1 = audio_buffer.clone();
    let buf2 = audio_buffer.clone();

    let (stream, actual_rate, actual_channels) = match device.build_input_stream(
        &preferred,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Ok(mut b) = buf1.lock() {
                b.extend_from_slice(data);
            }
        },
        |err| eprintln!("Audio stream error: {err}"),
        None,
    ) {
        Ok(s) => (s, 16000u32, 1u16),
        Err(e) => {
            eprintln!("16 kHz mono not supported ({e}), falling back to device default");
            let default_cfg = device
                .default_input_config()
                .map_err(|e| format!("No supported audio config: {e}"))?;
            let rate = default_cfg.sample_rate().0;
            let channels = default_cfg.channels();
            let stream_cfg = default_cfg.config();
            let s = device
                .build_input_stream(
                    &stream_cfg,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if let Ok(mut b) = buf2.lock() {
                            b.extend_from_slice(data);
                        }
                    },
                    |err| eprintln!("Audio stream error: {err}"),
                    None,
                )
                .map_err(|e| format!("Failed to open audio stream: {e}"))?;
            (s, rate, channels)
        }
    };

    *CAPTURE_RATE.lock().map_err(|e| e.to_string())? = actual_rate;
    *CAPTURE_CHANNELS.lock().map_err(|e| e.to_string())? = actual_channels;

    stream.play().map_err(|e| e.to_string())?;

    {
        let mut active = ACTIVE_STREAM.lock().map_err(|e| e.to_string())?;
        *active = Some(StreamHandle(stream));
    }

    set_state(app, &state, RecordingState::Recording);
    Ok(())
}

pub async fn stop_recording_internal(app: tauri::AppHandle) -> Result<(), String> {
    let t_release = Instant::now();
    *PTT_RELEASE_TIME.lock().map_err(|e| e.to_string())? = Some(t_release);

    {
        let mut active = ACTIVE_STREAM.lock().map_err(|e| e.to_string())?;
        *active = None;
    }
    let t_stream_dropped = Instant::now();

    let capture_rate = *CAPTURE_RATE.lock().map_err(|e| e.to_string())?;
    let capture_channels = *CAPTURE_CHANNELS.lock().map_err(|e| e.to_string())?;

    let (audio, backend, local_model, groq_api_key) = {
        let state = app.state::<AppState>();
        let raw_audio = state
            .audio_buffer
            .lock()
            .map_err(|e| e.to_string())?
            .clone();

        if raw_audio.is_empty() {
            set_state(&app, &state, RecordingState::Listening);
            return Ok(());
        }

        let mono = if capture_channels > 1 {
            to_mono(&raw_audio, capture_channels)
        } else {
            raw_audio
        };
        let audio = if capture_rate != 16000 {
            resample(&mono, capture_rate, 16000)
        } else {
            mono
        };

        let t_preprocess_done = Instant::now();
        eprintln!(
            "[TIMING] stream_drop={}ms preprocess={}ms audio_samples={}",
            t_stream_dropped.duration_since(t_release).as_millis(),
            t_preprocess_done.duration_since(t_stream_dropped).as_millis(),
            audio.len()
        );

        let (backend, local_model, groq_api_key) = {
            let s = state.settings.lock().map_err(|e| e.to_string())?;
            (s.backend.clone(), s.local_model.clone(), s.groq_api_key.clone())
        };
        set_state(&app, &state, RecordingState::Transcribing);
        (audio, backend, local_model, groq_api_key)
    };

    let t_transcribe_start = Instant::now();
    let result =
        crate::transcription::transcribe(audio, backend, local_model, groq_api_key, &app).await;
    let transcribe_elapsed = t_transcribe_start.elapsed().as_millis() as u64;

    let state = app.state::<AppState>();
    match result {
        Ok((text, timing)) => {
            let t_inject_start = Instant::now();
            if let Err(e) = crate::injector::inject_text(&text) {
                eprintln!("Injection error: {e}");
                let _ = app.emit("transcription-error", serde_json::json!({ "error": e }));
            }
            let inject_ms = t_inject_start.elapsed().as_millis() as u64;
            let total_ms = t_release.elapsed().as_millis() as u64;

            eprintln!(
                "[TIMING] transcribe={}ms inject={}ms TOTAL={}ms",
                transcribe_elapsed, inject_ms, total_ms
            );

            let _ = app.emit(
                "transcription-complete",
                serde_json::json!({ "text": text, "timing": timing }),
            );
        }
        Err(e) => {
            eprintln!("Transcription error: {e}");
            let _ = app.emit("transcription-error", serde_json::json!({ "error": e }));
        }
    }

    set_state(&app, &state, RecordingState::Listening);
    Ok(())
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    samples
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).ceil() as usize;
    (0..out_len)
        .map(|i| {
            let src = i as f64 * ratio;
            let lo = src as usize;
            let hi = (lo + 1).min(samples.len().saturating_sub(1));
            let t = (src - lo as f64) as f32;
            samples[lo] * (1.0 - t) + samples[hi] * t
        })
        .collect()
}

#[tauri::command]
pub fn start_recording(app: tauri::AppHandle) -> Result<(), String> {
    start_recording_internal(&app)
}

#[tauri::command]
pub async fn stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    stop_recording_internal(app).await
}
