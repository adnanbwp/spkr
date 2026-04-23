<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';

  type RecordingState = 'Inactive' | 'Listening' | 'Recording' | 'Transcribing';

  let currentState: RecordingState = 'Inactive';
  let unlisten: (() => void) | null = null;

  onMount(async () => {
    unlisten = await listen<{ state: RecordingState }>('state-changed', (event) => {
      currentState = event.payload.state;
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  $: circleColor = (() => {
    switch (currentState) {
      case 'Listening':    return '#22c55e';
      case 'Recording':    return '#ef4444';
      case 'Transcribing': return '#f97316';
      default:             return 'transparent';
    }
  })();

  $: isPulsing = currentState === 'Recording';
  $: isVisible = currentState !== 'Inactive';
</script>

{#if isVisible}
  <div class="container">
    <div
      class="circle"
      class:pulse={isPulsing}
      style="background-color: {circleColor};"
    >
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
    </div>
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
    pointer-events: none;
    width: 80px;
    height: 80px;
  }

  .container {
    width: 80px;
    height: 80px;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .circle {
    width: 60px;
    height: 60px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .circle.pulse {
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%   { transform: scale(1);    opacity: 1; }
    50%  { transform: scale(1.15); opacity: 0.8; }
    100% { transform: scale(1);    opacity: 1; }
  }
</style>
