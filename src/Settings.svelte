<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';

  type TranscriptionBackend = 'Local' | 'Groq';
  type LocalModel = 'Base' | 'Small' | 'Medium';

  interface AppSettings {
    backend: TranscriptionBackend;
    groq_api_key: string;
    local_model: LocalModel;
    input_device: string | null;
    toggle_app_hotkey: string | null;
    global_hotkey: string | null;
    ptt_hotkey: string | null;
  }

  interface ModelInfo {
    model: string;
    filename: string;
    downloaded: boolean;
    size_bytes: number | null;
  }

  interface ProgressPayload {
    model: string;
    downloaded: number;
    total: number;
    fraction: number;
  }

  const MODEL_META: Record<string, { label: string; est_mb: number }> = {
    Base:   { label: 'Base',   est_mb: 75  },
    Small:  { label: 'Small',  est_mb: 242 },
    Medium: { label: 'Medium', est_mb: 769 },
  };

  let settings: AppSettings = {
    backend: 'Local',
    groq_api_key: '',
    local_model: 'Base',
    input_device: null,
    toggle_app_hotkey: null,
    global_hotkey: null,
    ptt_hotkey: null,
  };

  let devices: string[] = [];
  let modelInfos: ModelInfo[] = [];
  let downloading: Record<string, boolean> = {};
  let progress: Record<string, number> = {};
  let status = '';
  let statusIsError = false;

  let unlistenProgress: (() => void) | undefined;
  let unlistenComplete: (() => void) | undefined;

  onMount(async () => {
    try {
      [settings, devices, modelInfos] = await Promise.all([
        invoke<AppSettings>('get_settings'),
        invoke<string[]>('list_input_devices'),
        invoke<ModelInfo[]>('list_models'),
      ]);
    } catch (e) {
      showStatus(`Failed to load: ${e}`, true);
    }

    unlistenProgress = await listen<ProgressPayload>('download-progress', ({ payload }) => {
      progress = { ...progress, [payload.model]: payload.fraction };
    });

    unlistenComplete = await listen<{ model: string }>('download-complete', async ({ payload }) => {
      downloading = { ...downloading, [payload.model]: false };
      progress = { ...progress, [payload.model]: 0 };
      modelInfos = await invoke<ModelInfo[]>('list_models');
    });
  });

  onDestroy(() => {
    unlistenProgress?.();
    unlistenComplete?.();
  });

  async function startDownload(model: string) {
    downloading = { ...downloading, [model]: true };
    progress = { ...progress, [model]: 0 };
    try {
      await invoke('download_model', { model });
    } catch (e) {
      downloading = { ...downloading, [model]: false };
      showStatus(`Download failed: ${e}`, true);
    }
  }

  async function deleteModel(model: string) {
    try {
      await invoke('delete_model', { model });
      modelInfos = await invoke<ModelInfo[]>('list_models');
    } catch (e) {
      showStatus(`Delete failed: ${e}`, true);
    }
  }

  function recordHotkey(e: KeyboardEvent, field: 'toggle_app_hotkey' | 'global_hotkey' | 'ptt_hotkey') {
    e.preventDefault();
    const parts: string[] = [];
    if (e.ctrlKey) parts.push('Ctrl');
    if (e.shiftKey) parts.push('Shift');
    if (e.altKey) parts.push('Alt');
    if (e.metaKey) parts.push('Super');
    const KEY_NAMES: Record<string, string> = { ' ': 'Space' };
    const raw = e.key;
    const key = KEY_NAMES[raw] ?? raw;
    if (key && !['Control', 'Shift', 'Alt', 'Meta'].includes(key)) {
      parts.push(key.length === 1 ? key.toUpperCase() : key);
    }
    if (parts.length > 0) settings = { ...settings, [field]: parts.join('+') };
  }

  function clearHotkey(field: 'toggle_app_hotkey' | 'global_hotkey' | 'ptt_hotkey') {
    settings = { ...settings, [field]: null };
  }

  async function save() {
    try {
      await invoke('save_settings', { settings });
      showStatus('Settings saved.', false);
    } catch (e) {
      showStatus(`Error: ${e}`, true);
    }
  }

  function showStatus(msg: string, isError: boolean) {
    status = msg;
    statusIsError = isError;
    if (!isError) setTimeout(() => (status = ''), 2500);
  }

  function formatBytes(n: number): string {
    if (n >= 1e9) return (n / 1e9).toFixed(1) + ' GB';
    if (n >= 1e6) return (n / 1e6).toFixed(0) + ' MB';
    return (n / 1e3).toFixed(0) + ' KB';
  }
</script>

<div class="settings">
  <h2>spkr Settings</h2>

  <!-- Backend -->
  <section>
    <span class="section-label">Transcription Backend</span>
    <div class="radio-group">
      <label><input type="radio" bind:group={settings.backend} value="Local" /> Local (Whisper)</label>
      <label><input type="radio" bind:group={settings.backend} value="Groq" /> Groq API</label>
    </div>
  </section>

  <!-- Groq API key -->
  {#if settings.backend === 'Groq'}
    <section>
      <label class="section-label" for="groq-key">Groq API Key</label>
      <input id="groq-key" type="password" bind:value={settings.groq_api_key}
        placeholder="gsk_..." class="text-input" />
    </section>
  {/if}

  <!-- Local model -->
  {#if settings.backend === 'Local'}
    <section>
      <span class="section-label">Local Model</span>
      <div class="model-list">
        {#each ['Base', 'Small', 'Medium'] as m (m)}
          {@const info = modelInfos.find(i => i.model === m)}
          {@const meta = MODEL_META[m]}
          {@const pct = progress[m] ?? 0}
          {@const isDownloading = downloading[m] ?? false}
          <div class="model-row">
            <label class="model-label">
              <input type="radio" bind:group={settings.local_model} value={m}
                disabled={!info?.downloaded} />
              <span>{meta.label}</span>
              <span class="model-size">~{meta.est_mb} MB</span>
              {#if info?.downloaded}
                <span class="badge-downloaded">
                  ✓ {info.size_bytes ? formatBytes(info.size_bytes) : 'Downloaded'}
                </span>
              {/if}
            </label>

            <div class="model-actions">
              {#if isDownloading}
                <div class="progress-wrap">
                  <div class="progress-bar">
                    <div class="progress-fill" style="width:{(pct * 100).toFixed(1)}%"></div>
                  </div>
                  <span class="progress-pct">{(pct * 100).toFixed(0)}%</span>
                </div>
              {:else if info?.downloaded}
                <button class="btn-ghost btn-danger" on:click={() => deleteModel(m)}>Delete</button>
              {:else}
                <button class="btn-secondary" on:click={() => startDownload(m)}>Download</button>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Input device -->
  <section>
    <label class="section-label" for="input-device">Input Device</label>
    {#if devices.length === 0}
      <p class="hint">No input devices found.</p>
    {:else}
      <select id="input-device" bind:value={settings.input_device} class="select-input">
        <option value={null}>— Default —</option>
        {#each devices as device}
          <option value={device}>{device}</option>
        {/each}
      </select>
    {/if}
  </section>

  <!-- Hotkeys -->
  <section>
    <span class="section-label">Toggle App On/Off</span>
    <div class="hotkey-row">
      <input type="text" readonly value={settings.toggle_app_hotkey ?? ''}
        on:keydown={(e) => recordHotkey(e, 'toggle_app_hotkey')}
        placeholder="Click and press keys…" class="text-input hotkey-input" />
      <button class="btn-ghost" on:click={() => clearHotkey('toggle_app_hotkey')}>Clear</button>
    </div>
  </section>

  <section>
    <span class="section-label">Global Hotkey (toggle record)</span>
    <div class="hotkey-row">
      <input type="text" readonly value={settings.global_hotkey ?? ''}
        on:keydown={(e) => recordHotkey(e, 'global_hotkey')}
        placeholder="Click and press keys…" class="text-input hotkey-input" />
      <button class="btn-ghost" on:click={() => clearHotkey('global_hotkey')}>Clear</button>
    </div>
  </section>

  <section>
    <span class="section-label">Push-to-Talk Hotkey</span>
    <div class="hotkey-row">
      <input type="text" readonly value={settings.ptt_hotkey ?? ''}
        on:keydown={(e) => recordHotkey(e, 'ptt_hotkey')}
        placeholder="Click and press keys…" class="text-input hotkey-input" />
      <button class="btn-ghost" on:click={() => clearHotkey('ptt_hotkey')}>Clear</button>
    </div>
  </section>

  <!-- Save -->
  <div class="actions">
    <button class="btn-primary" on:click={save}>Save Settings</button>
    {#if status}
      <span class="status" class:error={statusIsError}>{status}</span>
    {/if}
  </div>
</div>

<style>
  .settings {
    padding: 24px 28px;
    color: #e5e5e5;
    font-family: system-ui, sans-serif;
    font-size: 14px;
  }

  h2 {
    margin: 0 0 20px;
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }

  section {
    margin-bottom: 20px;
  }

  .section-label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #9ca3af;
    margin-bottom: 8px;
  }

  label.section-label {
    display: block;
  }

  .radio-group {
    display: flex;
    gap: 20px;
  }

  .radio-group label,
  .model-label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    color: #e5e5e5;
  }

  .model-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .model-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .model-label {
    flex: 1;
    min-width: 0;
  }

  .model-size {
    color: #6b7280;
    font-size: 12px;
  }

  .badge-downloaded {
    font-size: 11px;
    color: #22c55e;
    background: rgba(34, 197, 94, 0.1);
    padding: 1px 6px;
    border-radius: 4px;
    white-space: nowrap;
  }

  .model-actions {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .progress-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .progress-bar {
    width: 100px;
    height: 6px;
    background: #374151;
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #6366f1;
    border-radius: 3px;
    transition: width 0.2s ease;
  }

  .progress-pct {
    font-size: 12px;
    color: #9ca3af;
    width: 32px;
    text-align: right;
  }

  .text-input,
  .select-input {
    width: 100%;
    padding: 8px 10px;
    background: #1a1a1a;
    border: 1px solid #374151;
    border-radius: 6px;
    color: #e5e5e5;
    font-size: 14px;
    box-sizing: border-box;
    outline: none;
  }

  .text-input:focus,
  .select-input:focus {
    border-color: #6366f1;
  }

  .hotkey-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .hotkey-input {
    flex: 1;
    cursor: pointer;
  }

  .hotkey-input:focus {
    border-color: #6366f1;
    background: #1f1f2e;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 16px;
    padding-top: 8px;
    border-top: 1px solid #1f2937;
  }

  .btn-primary {
    padding: 8px 20px;
    background: #6366f1;
    color: #fff;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
  }

  .btn-primary:hover { background: #4f52d6; }

  .btn-secondary {
    padding: 4px 12px;
    background: transparent;
    border: 1px solid #4f46e5;
    border-radius: 4px;
    color: #818cf8;
    font-size: 12px;
    cursor: pointer;
  }

  .btn-secondary:hover {
    background: rgba(99, 102, 241, 0.1);
  }

  .btn-ghost {
    padding: 6px 12px;
    background: transparent;
    border: 1px solid #374151;
    border-radius: 6px;
    color: #9ca3af;
    font-size: 13px;
    cursor: pointer;
  }

  .btn-ghost:hover {
    border-color: #6b7280;
    color: #e5e5e5;
  }

  .btn-danger:hover {
    border-color: #ef4444;
    color: #ef4444;
  }

  .status {
    font-size: 13px;
    color: #22c55e;
  }

  .status.error { color: #ef4444; }

  .hint {
    color: #6b7280;
    font-size: 13px;
    margin: 0;
  }

  input[type='radio'] { accent-color: #6366f1; }
  select option { background: #1a1a1a; }
</style>
