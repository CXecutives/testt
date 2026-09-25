<!--
  Caption buttons of the Windows window, drawn like the native Windows 11 ones: Segoe Fluent
  Icons glyphs at 10 px, 46 px wide, the full height of the 36 px title bar, square, not in
  the tab order (like the native ones); only a quick colour change under the pointer, the
  close button red like on Windows. Pressed and moved off, a button looks at rest again and
  does nothing (a click needs the release on it). While the window is inactive the glyphs
  fade like the native ones. A short hover over maximize opens the snap layouts of Windows
  11 (they cannot see a button in the page by themselves). macOS keeps its traffic lights.
-->
<script lang="ts">
  import { t } from '$lib/i18n/t';
  import { appWindow } from '$lib/ipc/api';
  import { tokenMs } from '$lib/tokens';

  interface Props {
    testid?: string | null;
  }

  let { testid = 'window-controls' }: Props = $props();

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

  // The snap layouts after the pointer rests on maximize (like the native button's delay).
  let snapTimer: ReturnType<typeof setTimeout> | null = null;

  function snapSoon(): void {
    snapLater();
    snapTimer = setTimeout(() => {
      snapTimer = null;
      void appWindow.showSnapLayouts();
    }, tokenMs('--snap-delay'));
  }

  function snapLater(): void {
    if (snapTimer !== null) clearTimeout(snapTimer);
    snapTimer = null;
  }

  $effect(() => snapLater);
</script>

<div class="controls" data-testid={testid}>
  <button
    type="button"
    class="caption"
    tabindex="-1"
    aria-label={t.window.minimize}
    data-testid="window-minimize"
    onclick={() => void appWindow.minimize()}
  >
    <span class="glyph" aria-hidden="true">{GLYPH.minimize}</span>
  </button>
  <button
    type="button"
    class="caption"
    tabindex="-1"
    aria-label={maximized ? t.window.restore : t.window.maximize}
    data-testid="window-maximize"
    onpointerenter={snapSoon}
    onpointerleave={snapLater}
    onpointerdown={snapLater}
    onclick={() => {
      snapLater();
      void appWindow.toggleMaximize();
    }}
  >
    <span class="glyph" aria-hidden="true">{maximized ? GLYPH.restore : GLYPH.maximize}</span>
  </button>
  <button
    type="button"
    class="caption close"
    tabindex="-1"
    aria-label={t.window.close}
    data-testid="window-close"
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

  /* The window without the focus (`data-window` on the root, lib/platform.ts). */
  :global([data-window='inactive']) .caption {
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
    color: var(--caption-glyph-rest);
    transition-duration: var(--dur-hover);
  }

  :global(:where(:root:not([data-aux-press]))) .caption:active:hover {
    background-color: var(--caption-press);
  }

  .close:hover {
    background-color: var(--caption-close);
    color: var(--caption-close-glyph);
  }

  :global(:where(:root:not([data-aux-press]))) .close:active:hover {
    background-color: var(--caption-close-press);
    color: var(--caption-close-glyph);
  }
</style>
