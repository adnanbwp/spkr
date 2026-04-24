<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';

  type RecordingState = 'Inactive' | 'Listening' | 'Recording' | 'Transcribing';

  interface TranscriptionTiming {
    model_load_ms: number;
    inference_ms: number;
    segment_collect_ms: number;
    total_transcription_ms: number;
  }

  interface TranscriptionCompletePayload {
    text: string;
    timing?: TranscriptionTiming;
  }

  let currentState: RecordingState = 'Inactive';
  let errorFlash = false;
  let errorTimer: ReturnType<typeof setTimeout> | null = null;

  let timingText = '';
  let timingTimer: ReturnType<typeof setTimeout> | null = null;

  let unlistenState: (() => void) | null = null;
  let unlistenError: (() => void) | null = null;
  let unlistenComplete: (() => void) | null = null;

  onMount(async () => {
    unlistenState = await listen<{ state: RecordingState }>('state-changed', (event) => {
      currentState = event.payload.state;
    });

    unlistenError = await listen('transcription-error', () => {
      errorFlash = true;
      if (errorTimer) clearTimeout(errorTimer);
      errorTimer = setTimeout(() => { errorFlash = false; }, 2000);
    });

    unlistenComplete = await listen<TranscriptionCompletePayload>('transcription-complete', (event) => {
      const t = event.payload.timing;
      if (t) {
        timingText = `load ${t.model_load_ms}ms · infer ${t.inference_ms}ms · total ${t.total_transcription_ms}ms`;
        if (timingTimer) clearTimeout(timingTimer);
        timingTimer = setTimeout(() => { timingText = ''; }, 5000);
      }
    });
  });

  onDestroy(() => {
    unlistenState?.();
    unlistenError?.();
    unlistenComplete?.();
    if (errorTimer) clearTimeout(errorTimer);
    if (timingTimer) clearTimeout(timingTimer);
  });

  $: circleColor = (() => {
    if (errorFlash) return '#ef4444';
    switch (currentState) {
      case 'Listening':    return '#22c55e';
      case 'Recording':    return '#ef4444';
      case 'Transcribing': return '#f97316';
      default:             return 'transparent';
    }
  })();

  $: isPulsing = currentState === 'Recording' && !errorFlash;
  $: isErrorFlashing = errorFlash;
  $: isVisible = currentState !== 'Inactive' || errorFlash;
</script>

{#if isVisible}
  <!-- Outer row: timing on the left, mic on the right -->
  <div class="container">
    {#if timingText}
      <div class="timing">{timingText}</div>
    {/if}

    <div
      class="circle"
      class:pulse={isPulsing}
      class:error-flash={isErrorFlashing}
      style="background-color: {circleColor};"
    >
      {#if isErrorFlashing}
        <!-- Exclamation mark for error state -->
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="32"
          height="32"
          viewBox="0 0 24 24"
          fill="white"
          aria-hidden="true"
        >
          <path d="M12 2a10 10 0 1 0 0 20A10 10 0 0 0 12 2zm0 15a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3zm1-5.5h-2V7h2v4.5z"/>
        </svg>
      {:else}
        <!-- Microphone SVG icon -->
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="32"
          height="32"
          viewBox="0 0 24 24"
          fill="white"
          aria-hidden="true"
        >
          <!-- Mic body -->
          <rect x="9" y="2" width="6" height="11" rx="3" ry="3" />
          <!-- Mic stand arc -->
          <path d="M5 10a7 7 0 0 0 14 0" fill="none" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
          <!-- Vertical stem -->
          <line x1="12" y1="17" x2="12" y2="21" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
          <!-- Base line -->
          <line x1="9" y1="21" x2="15" y2="21" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      {/if}
    </div>
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }

  /* Full-width row: timing pill on the left, mic circle on the right */
  .container {
    width: 320px;
    height: 80px;
    display: flex;
    flex-direction: row;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    pointer-events: none;
  }

  .circle {
    flex-shrink: 0;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .timing {
    white-space: nowrap;
    font-size: 10px;
    font-family: monospace;
    color: rgba(255, 255, 255, 0.9);
    background: rgba(0, 0, 0, 0.55);
    border-radius: 4px;
    padding: 2px 6px;
    pointer-events: none;
  }

  .circle.pulse {
    animation: pulse 1s ease-in-out infinite;
  }

  .circle.error-flash {
    animation: error-shake 0.4s ease-out;
  }

  @keyframes pulse {
    0%   { transform: scale(1);    opacity: 1; }
    50%  { transform: scale(1.15); opacity: 0.8; }
    100% { transform: scale(1);    opacity: 1; }
  }

  @keyframes error-shake {
    0%   { transform: translateX(0); }
    20%  { transform: translateX(-5px); }
    40%  { transform: translateX(5px); }
    60%  { transform: translateX(-5px); }
    80%  { transform: translateX(5px); }
    100% { transform: translateX(0); }
  }
</style>
