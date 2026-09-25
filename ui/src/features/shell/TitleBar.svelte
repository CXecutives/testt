<!--
  The Windows title bar, drawn by the page like the one of the Claude app: 36 px across the
  full width above sidebar and sheet, in the app's cream, the app icon at 16 px and the app's
  name in Inter (12 px), the caption buttons at the right. The whole bar moves the window and
  a double click maximizes it (Tauri's drag region, `deep`: every child but the buttons);
  dragged to the top it maximizes, to the sides it snaps (Windows does that). While the
  window is inactive the name and the glyphs fade like on a native window. macOS keeps its
  traffic lights in the toolbar row: nothing here.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import WindowControls from '$components/WindowControls.svelte';
  import { t } from '$lib/i18n/t';
  import { ownTitleBar } from '$lib/platform';

  const own = ownTitleBar();
</script>

{#if own}
  <header class="bar" data-testid="titlebar" data-tauri-drag-region="deep">
    <span class="brand" data-testid="titlebar-brand">
      <BrandMark size="xs" />
      <span class="name">{t.app.name}</span>
    </span>
    <WindowControls />
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
    gap: var(--space-8);
    min-width: 0;
    padding-left: var(--space-12);
  }

  .name {
    overflow: hidden;
    color: var(--text);
    font: var(--type-window);
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--dur-fast) var(--ease-standard);
  }

  :global([data-window='inactive']) .name {
    color: var(--caption-inactive);
  }
</style>
