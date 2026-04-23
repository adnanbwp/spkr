<script lang="ts">
  import Overlay from './Overlay.svelte';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  // Use Tauri window label for routing — works correctly in both dev and prod.
  // Hash-based routing was unreliable in dev mode because Tauri sends all windows
  // to the same devUrl (http://localhost:1420) without the hash fragment.
  const isOverlay = getCurrentWebviewWindow().label === 'overlay';
</script>

{#if isOverlay}
  <Overlay />
{:else}
  <main>
    <h1>spkr Settings</h1>
  </main>
{/if}

<style>
  main {
    text-align: center;
    padding: 1em;
    background-color: #242424;
    min-height: 100vh;
  }
</style>
