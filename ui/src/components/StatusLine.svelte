<!--
  A quiet, clickable status line (the run status at the foot of the sidebar): an icon or a
  spinner, one short text and, while something runs, a slim meter below. Collapsed (icon
  rail) only the icon stays; the text moves into the tooltip.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import Icon, { type IconName } from './Icon.svelte';
  import Meter from './Meter.svelte';
  import Spinner from './Spinner.svelte';

  interface Props {
    text: string;
    /** What the click does (accessible name). */
    label: string;
    icon?: IconName;
    busy?: boolean;
    /** Progress 0..1, null for unknown; undefined shows no meter. */
    progress?: number | null | undefined;
    progressLabel?: string;
    tone?: 'neutral' | 'danger';
    collapsed?: boolean;
    testid?: string | null;
    onclick: () => void;
  }

  let {
    text,
    label,
    icon = 'clock',
    busy = false,
    progress,
    progressLabel = label,
    tone = 'neutral',
    collapsed = false,
    testid = null,
    onclick,
  }: Props = $props();
</script>

<button
  type="button"
  class="status {tone}"
  class:collapsed
  aria-label={collapsed ? `${label} ${text}` : label}
  data-testid={testid ?? undefined}
  use:tooltip={collapsed ? text : null}
  onclick={() => onclick()}
>
  <span class="line">
    <span class="glyph">
      {#if busy}<Spinner size="sm" label={null} />{:else}<Icon name={icon} size="sm" />{/if}
    </span>
    {#if !collapsed}<span class="text">{text}</span>{/if}
  </span>
  {#if progress !== undefined}
    <Meter value={progress} size="sm" label={progressLabel} />
  {/if}
</button>

<style>
  .status {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    width: 100%;
    padding: var(--space-8) var(--space-4) var(--space-8) var(--space-12);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font: var(--type-sm);
    text-align: left;
    transition:
      background-color var(--dur-fast) var(--ease-standard),
      color var(--dur-fast) var(--ease-standard);
  }

  .status:hover {
    background-color: var(--surface-hover);
    color: var(--text);
  }

  .status:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .danger,
  .danger:hover {
    color: var(--danger-strong);
  }

  .collapsed {
    align-items: center;
    width: var(--control-lg);
    padding: var(--space-12) 0;
  }

  .line {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    min-width: 0;
  }

  .glyph {
    display: inline-flex;
    flex: none;
  }

  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
