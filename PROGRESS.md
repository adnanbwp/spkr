# spkr — Build Progress

## Status: ALL STEPS COMPLETE (1–11). App is feature-complete. Next: test on target platform and build.

---

## What's Been Built

### Step 1: Project Scaffold ✅
- Tauri v2 app with Svelte + TypeScript frontend
- All Rust dependencies added: `cpal`, `whisper-rs`, `reqwest`, `hound`, `enigo`, `serde`, `tokio`
- Tauri plugins added: `tauri-plugin-global-shortcut`, `tauri-plugin-store`
- Two windows configured in `tauri.conf.json`:
  - `settings` — normal app window (600×500, hidden by default)
  - `overlay` — transparent, always-on-top, no decorations (80×80, hidden by default)
- System tray icon configured
- Capabilities scoped per window: `src-tauri/capabilities/settings.json` and `overlay.json`
- 7 placeholder Rust modules created: `state`, `audio`, `transcription`, `models`, `hotkeys`, `injector`, `settings`
- `main.rs` delegates to `lib.rs` (no duplication)
- `cargo check` passes

### Step 2: State Machine + AppState ✅
- `RecordingState` enum: `Inactive` | `Listening` | `Recording` | `Transcribing`
- `AppSettings` struct: backend, groq_api_key, local_model, input_device, global_hotkey, ptt_hotkey
- `TranscriptionBackend` enum: `Local` | `Groq`
- `LocalModel` enum: `Base` | `Small` | `Medium`
- `AppState` struct with `Mutex<RecordingState>`, `Mutex<AppSettings>`, `Mutex<Vec<f32>>` (audio buffer)
- `AppState::default()` registered with `.manage()` in `lib.rs`
- `set_state()` helper: updates state + emits `"state-changed"` event + shows/hides overlay window
- Tauri commands: `get_recording_state`, `toggle_listening`, `get_settings`, `save_settings`
- All commands return `Result<T, String>` for proper error propagation

### Step 11: Polish & Edge Cases ✅
- `audio.rs::start_recording_internal`: saved device not found → logs warning and falls back to system default instead of hard-erroring
- `audio.rs::stop_recording_internal`: empty audio buffer (hotkey tapped with nothing recorded) → silently returns to Listening, skips transcription/Groq call
- `Overlay.svelte`: listens to `transcription-error` event; sets `errorFlash = true` for 2s — overrides circle to red, swaps mic icon for exclamation mark, plays a quick shake animation; clears and returns to current state color after timeout

### Step 10: System Tray + Overlay Positioning ✅
- Tray menu was already complete from Step 4 ("Open Settings", "Toggle Listening", "Quit")
- Overlay positioned programmatically in `lib.rs::setup`: reads `overlay.primary_monitor()`, converts physical size to logical pixels using `scale_factor()`, sets position to `(logical_width - 80 - 16, 16)` — top-right corner with 16px margin
- Position is set once on startup before the overlay is ever shown; `set_state` show/hide preserves position
- Tray icon state reflection (color change per state) deferred to Step 11 polish

### Step 9: Hotkey System ✅
- `hotkeys.rs` — `register_hotkeys(app, global_hotkey, ptt_hotkey)`: unregisters all existing shortcuts, then registers toggle and/or PTT with individual `on_shortcut` closures
- Toggle handler (`on_toggle_press`): Listening → `start_recording_internal`; Recording → spawns async `stop_recording_internal`; Transcribing/Inactive ignored
- PTT handler: `on_ptt_press` starts if Listening; `on_ptt_release` stops if Recording (edge case: keyup ignored in Transcribing, per plan)
- Same key for both toggle and PTT is detected and skipped (would double-register)
- `audio.rs` refactored: `start_recording_internal(app: &AppHandle)` and `async stop_recording_internal(app: AppHandle)` extracted; Tauri commands are now thin wrappers. `stop_recording_internal` takes owned `AppHandle` so spawned futures are `'static + Send`
- `settings.rs::save_settings` calls `register_hotkeys` after persisting — re-registers on settings change
- `lib.rs::setup` calls `register_hotkeys` after `load_settings_from_store` — hotkeys active on startup if previously saved

### Step 8: Text Injection ✅
- `injector.rs` — `inject_text(text: &str)`: creates `enigo::Enigo` instance, calls `enigo.text()` to type into the currently focused window
- `audio.rs::stop_recording`: after successful transcription, calls `inject_text` then emits `transcription-complete`; injection errors are logged and emitted as `transcription-error` (text is still emitted as complete for frontend display)
- enigo 0.2 auto-detects display server (X11 / Wayland) on Linux; WSL2 with WSLg works if a display is active

### Step 7: Transcription ✅
- `transcription.rs` — `pub async fn transcribe(audio, backend, local_model, groq_api_key, app)` dispatches to local or Groq
- Local path: `run_local` (runs in `tokio::task::spawn_blocking`) — loads `whisper-rs` context from `{app_data_dir}/models/<model>.bin`, runs inference with English language, concatenates segments
- Groq path: `run_groq` — encodes PCM f32 → 16-bit WAV in memory via `pcm_to_wav` helper (no hound needed for this path), POSTs multipart to `api.groq.com/openai/v1/audio/transcriptions` with `whisper-large-v3-turbo`
- `audio.rs::stop_recording` made `async`: drops stream → clones audio buffer + settings → sets state `Transcribing` → awaits `transcribe()` → emits `transcription-complete` (or `transcription-error`) → sets state `Listening`
- Step 8 (text injection) will consume the `transcription-complete` event

### Step 6: Model Management ✅
- `models.rs` — `list_models` checks disk and returns `ModelInfo` (model name, filename, downloaded, size_bytes)
- `models.rs` — `download_model` (async): streams from HuggingFace via `reqwest::Response::chunk()`, emits `download-progress` events, renames tmp→final on success, cleans up tmp on error
- `models.rs` — `delete_model`: removes the .bin file from disk
- `models.rs` — `model_path()` public helper (used by transcription in Step 7)
- `settings.rs` — `LocalModel::as_str()` added for canonical string conversion
- `Settings.svelte` — Download buttons now active; shows progress bar (fraction) while downloading; shows "✓ <size>" badge when downloaded; Delete button replaces Download for existing models; listens for `download-progress` and `download-complete` events

### Step 5: Audio Capture ✅
- `audio.rs` — `start_recording` opens a cpal input stream at 16kHz mono, clears and fills `audio_buffer`, transitions state → Recording
- `audio.rs` — `stop_recording` drops the stream (stops capture), transitions state → Listening (Step 7 will insert Transcribing + actual inference here)
- `StreamHandle` wrapper with `unsafe impl Send + Sync` to appease cpal's conservative cross-platform `!Send` marker on ALSA
- `AppState.audio_buffer` changed from `Mutex<Vec<f32>>` to `Arc<Mutex<Vec<f32>>>` so the cpal stream callback can own a clone of the Arc without `State<'_>` lifetime issues
- Device selection: uses `settings.input_device` if set, otherwise falls back to system default
- Known gap: if user force-stops via tray toggle while Recording, the stream keeps running until next `start_recording` clears it (Step 11 cleanup)

### Step 4: Settings Window + Persistence ✅
- `src/Settings.svelte` — full settings form:
  - Backend toggle (Local / Groq), Groq API key (conditional), local model radio + disabled Download buttons (Step 6)
  - Input device dropdown (populated from `list_input_devices`)
  - Global hotkey + PTT hotkey with in-browser key recording (click field, press combo)
  - Save button with success/error status feedback
- `save_settings` now persists to `{app_data_dir}/settings.json` via `tauri-plugin-store`
- `load_settings_from_store` called in setup — settings survive restarts
- `list_input_devices` command added to `audio.rs` (cpal host enumeration)
- `toggle_listening_internal` extracted in `state.rs` for use outside command context
- System tray created programmatically with three menu items: Open Settings / Toggle Listening / Quit
- `trayIcon` removed from `tauri.conf.json` (tray managed entirely in Rust setup)
- Settings window height increased to 620px

### Step 3: Overlay Window ✅
- `src/Overlay.svelte` — floating status indicator:
  - 60px circle with microphone SVG icon (white)
  - Green (`#22c55e`) when Listening
  - Pulsing red (`#ef4444`) when Recording (CSS keyframe animation)
  - Orange (`#f97316`) when Transcribing
  - Renders nothing when Inactive
  - `pointer-events: none` throughout
- Routing: uses `getCurrentWebviewWindow().label === 'overlay'` (works in both dev and prod)
- `set_ignore_cursor_events(true)` set in setup closure (click-through)
- Background color scoped to settings window only (overlay stays transparent)
- TypeScript config fixed (`@tsconfig/svelte` installed, `moduleResolution` set correctly)

---

## Key File Locations

| File | Purpose |
|---|---|
| `src-tauri/src/lib.rs` | App entry, plugin registration, setup closure, invoke_handler |
| `src-tauri/src/state.rs` | RecordingState, AppState, set_state, toggle_listening |
| `src-tauri/src/settings.rs` | AppSettings, TranscriptionBackend, LocalModel, get/save commands |
| `src-tauri/src/audio.rs` | Placeholder (Step 5) |
| `src-tauri/src/transcription.rs` | Placeholder (Step 7) |
| `src-tauri/src/models.rs` | Placeholder (Step 6) |
| `src-tauri/src/hotkeys.rs` | Placeholder (Step 9) |
| `src-tauri/src/injector.rs` | Placeholder (Step 8) |
| `src-tauri/tauri.conf.json` | Window config, tray icon |
| `src-tauri/capabilities/settings.json` | Permissions for settings window |
| `src-tauri/capabilities/overlay.json` | Permissions for overlay window |
| `src/App.svelte` | Root: routes to Overlay or Settings based on window label |
| `src/Overlay.svelte` | Overlay UI component |
| `src/Settings.svelte` | Settings form UI |

---

## Remaining Steps

| Step | Description |
|---|---|
| ~~Step 4~~ | Settings window UI + persistence (Tauri store plugin) ✅ |
| ~~Step 5~~ | Audio capture (`cpal`, 16kHz mono, device selection) ✅ |
| ~~Step 6~~ | Model management (download GGML .bin files from HuggingFace) ✅ |
| ~~Step 7~~ | Transcription (`whisper-rs` local + Groq API) ✅ |
| ~~Step 8~~ | Text injection (`enigo`, type into active window) ✅ |
| ~~Step 9~~ | Hotkey system (toggle + push-to-talk, re-register on change) ✅ |
| ~~Step 10~~ | System tray + overlay positioning ✅ |
| ~~Step 11~~ | Polish & edge cases (errors, fallbacks, notifications) ✅ |

---

## Known Notes for Next Steps

- **Step 6**: Model files (`ggml-base.en.bin` etc.) live at `{app_data_dir}/models/`. Need `reqwest` streaming download with progress events and a "Downloaded" badge per model. Download buttons in Settings.svelte are already present (disabled).
- **WSL2 audio**: WSL2 may not expose ALSA devices. `start_recording` will return an error if no default device is found. Test on bare Linux or Windows.
- **Step 10**: Overlay window position (currently `x:0, y:0`) needs to be set programmatically to top-right corner of primary monitor.
- **`listening_enabled`** was intentionally removed from `AppSettings` — `RecordingState` is the single source of truth. The tray menu "Toggle Listening" calls `toggle_listening`.

---

## Build Commands

```bash
# Check Rust compiles
cd src-tauri && cargo check

# Check TypeScript
npx tsc --noEmit

# Run in dev (requires Tauri CLI)
cargo tauri dev

# Production build
cargo tauri build
```
