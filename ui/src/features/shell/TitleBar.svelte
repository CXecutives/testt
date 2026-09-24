<!--
  The 48 px title bar: brand, centered word tabs, caption buttons.
  Per-OS markup lives only here and in WindowControls: Windows draws its own caption
  buttons on the right; macOS keeps the traffic lights, which need an 80 px inset on the
  left. Every cell and gap carries data-tauri-drag-region (Tauri only drags on the element
  that has it, so decorative children ignore the pointer); the tab buttons do not.
-->
<script lang="ts">
  import Icon from '$components/Icon.svelte';
  import NavTabs, { type NavTab } from '$components/NavTabs.svelte';
  import WindowControls from '$components/WindowControls.svelte';
  import { de } from '$lib/i18n/de';
  import { platform } from '$lib/platform';
  import { navigation, type ViewId } from '$lib/state/navigation.svelte';

  const os = platform();

  const tabs: readonly NavTab<ViewId>[] = [
    { id: 'jobs', label: de.nav.jobs, testid: 'tab-jobs' },
    { id: 'profile', label: de.nav.profile, testid: 'tab-profile' },
    { id: 'settings', label: de.nav.settings, testid: 'tab-settings' },
  ];
</script>

<header class="titlebar {os}" data-testid="titlebar" data-tauri-drag-region>
  <div class="cell start" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region data-testid="brand">
      <span class="tile"><Icon name="sparkles" size="sm" /></span>
      <span class="name">{de.app.name}</span>
    </div>
  </div>
  <div class="cell center" data-tauri-drag-region>
    <NavTabs
      {tabs}
      active={navigation.current}
      label={de.nav.label}
      onselect={(id) => navigation.go(id)}
    />
  </div>
  <div class="cell end" data-tauri-drag-region>
    {#if os === 'windows'}
      <WindowControls />
    {/if}
  </div>
</header>

<style>
  .titlebar {
    position: relative;
    z-index: var(--z-titlebar);
    display: grid;
    flex: none;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    height: var(--titlebar-height);
    border-bottom: var(--border-width) solid var(--border);
    background-color: var(--surface);
  }

  .cell {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .start {
    padding-left: var(--space-16);
  }

  .macos .start {
    padding-left: var(--traffic-light-inset);
  }

  .center {
    align-items: stretch;
    justify-content: center;
  }

  .end {
    align-items: stretch;
    justify-content: flex-end;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  .brand > * {
    pointer-events: none;
  }

  .tile {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: var(--brand-tile);
    height: var(--brand-tile);
    border-radius: var(--radius-sm);
    background: var(--grad-coral);
    box-shadow: var(--sh-elegant);
    color: var(--text-on-accent);
  }

  .name {
    overflow: hidden;
    color: var(--text-heading);
    font: var(--type-md);
    font-weight: var(--weight-semibold);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (width < 900px) {
    .name {
      display: none;
    }
  }
</style>
