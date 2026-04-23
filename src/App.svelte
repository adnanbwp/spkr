<script lang="ts">
  import Overlay from './Overlay.svelte';
  import Settings from './Settings.svelte';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  const label = getCurrentWebviewWindow().label;
</script>

{#if label === 'overlay'}
  <Overlay />
{:else}
  <Settings />
{/if}

<style>
  :global(body) {
    margin: 0;
    background-color: #242424;
    min-height: 100vh;
  }
</style>
