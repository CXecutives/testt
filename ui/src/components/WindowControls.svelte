<!--
  Caption buttons of the frameless Windows window, drawn like the native Windows 11 ones:
  Segoe Fluent Icons glyphs at 10 px, 46 px wide, the full height of the 30 px title bar,
  square, no tooltip, not in the tab order (like the native ones); only a quick colour
  change on hover and press. While the window is inactive the glyphs fade like the native
  ones. macOS keeps its native title bar instead.
-->
<script lang="ts">
  import { de } from '$lib/i18n/de';
  import { appWindow } from '$lib/ipc/api';

  interface Props {
    /** The window does not have the focus: glyphs fade (as on native windows). */
    inactive?: boolean;
  }

  let { inactive = false }: Props = $props();

  /** Code points of the Windows icon font (Segoe Fluent Icons / Segoe MDL2 Assets). */
  const GLYPH = {
    minimize: '',
    maximize: '',
    restore: '',
    close: '',
  } as const;

  let maximized = $state(false);

  $effect(() => {
    void appWindow.isMaximized().then(
      (value) => {
        maximized = value;
      },
      () => undefined,
    );
    return appWindow.onMaximizedChange((value) => {
      maximized = value;
    });
  });
</script>

<div class="controls" class:inactive data-testid="window-controls">
  <button
    type="button"
    class="caption"
    tabindex="-1"
    aria-label={de.window.minimize}
    onclick={() => void appWindow.minimize()}
  >
    <span class="glyph" aria-hidden="true">{GLYPH.minimize}</span>
  </button>
  <button
    type="button"
    class="caption"
    tabindex="-1"
    aria-label={maximized ? de.window.restore : de.window.maximize}
    onclick={() => void appWindow.toggleMaximize()}
  >
    <span class="glyph" aria-hidden="true">{maximized ? GLYPH.restore : GLYPH.maximize}</span>
  </button>
  <button
    type="button"
    class="caption close"
    tabindex="-1"
    aria-label={de.window.close}
    onclick={() => void appWindow.close()}
  >
    <span class="glyph" aria-hidden="true">{GLYPH.close}</span>
  </button>
</div>

<style>
  .controls {
    display: flex;
    flex: none;
    align-self: stretch;
  }

  .caption {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--caption-width);
    height: var(--titlebar-height);
    border-radius: 0;
    background-color: transparent;
    color: var(--caption-glyph-rest);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .inactive .caption {
    color: var(--caption-inactive);
  }

  .glyph {
    font-family: var(--font-caption);
    font-size: var(--caption-glyph);
    font-weight: var(--weight-regular);
    line-height: var(--leading-none);
  }

  .caption:hover {
    background-color: var(--caption-hover);
    color: var(--caption-glyph-hover);
  }

  .caption:active {
    background-color: var(--caption-press);
    color: var(--caption-glyph-hover);
  }

  .close:hover {
    background-color: var(--caption-close);
    color: var(--caption-close-glyph);
  }

  .close:active {
    background-color: var(--caption-close-press);
    color: var(--caption-close-glyph);
  }
</style>
