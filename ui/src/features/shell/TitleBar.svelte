<!--
  The thin 40 px strip on top of the content: the drag region of the window, on Windows with
  the caption buttons on the right (flat, warm hover, warm red close). macOS keeps its traffic
  lights in the sidebar's top left, so the strip is only the drag region there.
  Tauri drags only on the element that carries data-tauri-drag-region.
-->
<script lang="ts">
  import WindowControls from '$components/WindowControls.svelte';
  import { platform } from '$lib/platform';

  const os = platform();
</script>

<header class="strip {os}" data-testid="titlebar" data-tauri-drag-region>
  {#if os === 'windows'}
    <WindowControls />
  {/if}
</header>

<style>
  .strip {
    position: relative;
    z-index: var(--z-titlebar);
    display: flex;
    flex: none;
    justify-content: flex-end;
    height: var(--titlebar-height);
    background-color: var(--bg);
  }
</style>
