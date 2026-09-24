<!--
  Caption buttons of the frameless Windows window, drawn like the native Windows 11 ones:
  Segoe Fluent Icons glyphs at 10 px, 46 px wide, full title-bar height, square, a quick
  colour change and no lift or scale. macOS keeps its native traffic lights instead.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import { appWindow } from '$lib/ipc/api';

  /** Code points of the Windows icon font (Segoe Fluent Icons / Segoe MDL2 Assets). */
  const GLYPH = {
    minimize: '',
    maximize: '',
    restore: '',
    close: '',
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

  const maximizeLabel = $derived(maximized ? de.window.restore : de.window.maximize);
</script>

<div class="controls" data-testid="window-controls">
  <button
    type="button"
    class="caption"
    aria-label={de.window.minimize}
    use:tooltip={de.window.minimize}
    onclick={() => void appWindow.minimize()}
  >
    <span class="glyph" aria-hidden="true">{GLYPH.minimize}</span>
  </button>
  <button
    type="button"
    class="caption"
    aria-label={maximizeLabel}
    use:tooltip={maximizeLabel}
    onclick={() => void appWindow.toggleMaximize()}
  >
    <span class="glyph" aria-hidden="true">{maximized ? GLYPH.restore : GLYPH.maximize}</span>
  </button>
  <button
    type="button"
    class="caption close"
    aria-label={de.window.close}
    use:tooltip={de.window.close}
    onclick={() => void appWindow.close()}
  >
    <span class="glyph" aria-hidden="true">{GLYPH.close}</span>
  </button>
</div>

<style>
  .controls {
    display: flex;
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

  .caption:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }
</style>
