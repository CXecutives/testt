<!--
  Own caption buttons for the frameless Windows window (macOS keeps its traffic lights).
  46 × 48 like Windows 11; close turns red on hover.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { de } from '$lib/i18n/de';
  import { appWindow } from '$lib/ipc/api';
  import Icon from './Icon.svelte';

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
    <Icon name="minus" size="sm" />
  </button>
  <button
    type="button"
    class="caption"
    aria-label={maximizeLabel}
    use:tooltip={maximizeLabel}
    onclick={() => void appWindow.toggleMaximize()}
  >
    <Icon name={maximized ? 'copy' : 'square'} size="sm" />
  </button>
  <button
    type="button"
    class="caption close"
    aria-label={de.window.close}
    use:tooltip={de.window.close}
    onclick={() => void appWindow.close()}
  >
    <Icon name="x" size="sm" />
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
    color: var(--text-muted);
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .caption:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .caption:active {
    background-color: var(--surface-press);
  }

  .close:hover {
    background-color: var(--caption-close);
    color: var(--text-on-accent);
  }

  .close:active {
    background-color: var(--caption-close-active);
    color: var(--text-on-accent);
  }

  .caption:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }
</style>
