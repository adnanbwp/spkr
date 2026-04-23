<script lang="ts">
  import Overlay from './Overlay.svelte';

  // Hash-based routing: #overlay → overlay window, everything else → settings UI
  const hash = typeof window !== 'undefined' ? window.location.hash : '';
  const isOverlay = hash === '#overlay';
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
