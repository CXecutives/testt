<!--
  The Windows title bar, like a native Windows 11 one in the app's colours: full width above
  sidebar and content, 30 px (below the 1 px window frame), the app icon at 16 px 8 px from
  the edge, the app name in the system caption font (12 px, regular), the caption buttons
  at the right. The whole bar drags the window and a double click maximizes it (Tauri's
  drag region, `deep`: every child but the buttons). While the window is inactive title and
  glyphs fade like on a native window. macOS keeps its native title bar: nothing here.
  Per-OS markup lives only in the shell and WindowControls.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import WindowControls from '$components/WindowControls.svelte';
  import { de } from '$lib/i18n/de';
  import { platform } from '$lib/platform';

  const os = platform();
  let active = $state(document.hasFocus());

  $effect(() => {
    const focus = (): void => void (active = true);
    const blur = (): void => void (active = false);
    addEventListener('focus', focus);
    addEventListener('blur', blur);
    return () => {
      removeEventListener('focus', focus);
      removeEventListener('blur', blur);
    };
  });
</script>

{#if os === 'windows'}
  <header class="bar" class:inactive={!active} data-testid="titlebar" data-tauri-drag-region="deep">
    <span class="brand" data-testid="brand">
      <BrandMark size="xs" />
      <span class="name">{de.app.name}</span>
    </span>
    <WindowControls inactive={!active} />
  </header>
{/if}

<style>
  .bar {
    position: relative;
    z-index: var(--z-titlebar);
    display: flex;
    flex: none;
    align-items: center;
    justify-content: space-between;
    height: var(--titlebar-height);
    background-color: var(--bg);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
    padding-left: var(--space-8);
  }

  .name {
    overflow: hidden;
    color: var(--text);
    font: var(--type-window);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .inactive .name {
    color: var(--caption-inactive);
  }
</style>
