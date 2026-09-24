<!--
  The thin 40 px strip on top of the content and the drag region of the window: "Abrufen" at
  the left as the window's compact primary ("Abbrechen" during a run), on Windows the caption
  buttons at the right. macOS keeps its traffic lights in the sidebar's top left.
  Tauri drags only on the element that carries data-tauri-drag-region, so the buttons do not.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import WindowControls from '$components/WindowControls.svelte';
  import { de } from '$lib/i18n/de';
  import { platform } from '$lib/platform';
  import { app } from '$lib/state/app.svelte';
  import { run } from '$lib/state/run.svelte';
  import { shell } from '$lib/state/shell.svelte';

  const os = platform();
</script>

<header class="strip {os}" data-testid="titlebar" data-tauri-drag-region>
  <div class="fetch">
    {#if shell.firstRun && !run.active}
      <!-- The first-run page walks through its own "Abrufen". -->
    {:else if run.active}
      <Button
        variant="secondary"
        size="sm"
        icon="square"
        label={de.toolbar.cancel}
        loading={run.cancelling}
        testid="cancel-run"
        onclick={() => void run.cancel()}
      />
    {:else}
      <Button
        variant={app.hasMailbox ? 'primary' : 'secondary'}
        size="sm"
        icon="refresh-cw"
        label={de.toolbar.fetch}
        disabled={!app.hasMailbox}
        disabledReason={de.toolbar.needsMailbox}
        testid="fetch"
        onclick={() => void run.start({ kind: 'fetch' })}
      />
    {/if}
  </div>
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
    align-items: center;
    justify-content: space-between;
    height: var(--titlebar-height);
    padding-left: var(--pane-padding);
    background-color: var(--bg);
  }

  .fetch {
    display: flex;
    align-items: center;
  }
</style>
