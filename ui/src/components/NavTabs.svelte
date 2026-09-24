<!--
  Word tabs with a gliding 2 px gradient marker. The marker moves by transform only
  (translateX + scaleX of a full-width bar), measured in whole pixels.
  The nav element carries data-tauri-drag-region so the gaps between tabs drag the window;
  the tab buttons do not.
-->
<script lang="ts" module>
  export interface NavTab<Id extends string = string> {
    id: Id;
    label: string;
    testid?: string;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { cssVars, px } from '$lib/actions/cssVars';

  interface Props {
    tabs: readonly NavTab<Id>[];
    active: Id;
    label: string;
    onselect: (id: Id) => void;
  }

  let { tabs, active, label, onselect }: Props = $props();

  let nav = $state<HTMLElement | null>(null);
  let x = $state(0);
  let width = $state(0);
  let navWidth = $state(1);
  let ready = $state(false);

  function measure(): void {
    if (nav === null) return;
    const tab = nav.querySelector<HTMLElement>(`[data-tab="${CSS.escape(active)}"]`);
    if (tab === null) return;
    x = tab.offsetLeft;
    width = tab.offsetWidth;
    navWidth = Math.max(nav.clientWidth, 1);
  }

  $effect(() => {
    void active;
    measure();
  });

  $effect(() => {
    if (nav === null) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(nav);
    // Glide only after the first placement - never from the left edge on start.
    const frame = requestAnimationFrame(() => {
      ready = true;
    });
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  });
</script>

<nav class="tabs" aria-label={label} bind:this={nav} data-tauri-drag-region>
  <div class="list" role="tablist" data-tauri-drag-region>
    {#each tabs as tab (tab.id)}
      <button
        type="button"
        role="tab"
        class="tab"
        aria-selected={tab.id === active}
        data-tab={tab.id}
        data-label={tab.label}
        data-testid={tab.testid}
        onclick={() => onselect(tab.id)}
      >
        <span class="text">{tab.label}</span>
      </button>
    {/each}
  </div>
  <span
    class="marker"
    class:ready
    aria-hidden="true"
    use:cssVars={{ 'marker-x': px(x), 'marker-scale': width / navWidth }}
  ></span>
</nav>

<style>
  .tabs {
    position: relative;
    display: flex;
    align-self: stretch;
  }

  .list {
    display: flex;
    align-items: stretch;
    gap: var(--space-4);
  }

  .tab {
    position: relative;
    display: inline-grid;
    align-items: center;
    padding: 0 var(--space-12);
    color: var(--text-muted);
    font: var(--type-tab);
    transition:
      color var(--dur-fast) var(--ease-standard),
      transform var(--dur-fast) var(--ease-pop);
  }

  /* The bold label reserves its width, so switching weight never shifts the tabs. */
  .text,
  .tab::after {
    grid-area: 1 / 1;
  }

  .tab::after {
    height: 0;
    content: attr(data-label);
    font-weight: var(--weight-semibold);
    visibility: hidden;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab:active {
    transform: scale(var(--scale-press));
    transition-duration: var(--dur-instant);
  }

  .tab:focus-visible {
    border-radius: var(--radius-sm);
    box-shadow: var(--focus-ring-inset);
  }

  .tab[aria-selected='true'] {
    color: var(--text-heading);
    font-weight: var(--weight-semibold);
  }

  .marker {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: var(--marker-height);
    border-radius: var(--radius-full);
    background: var(--grad-brand);
    pointer-events: none;
    transform: translateX(var(--marker-x)) scaleX(var(--marker-scale));
    transform-origin: left center;
  }

  .marker.ready {
    transition: transform var(--dur-slow) var(--ease-out);
  }
</style>
