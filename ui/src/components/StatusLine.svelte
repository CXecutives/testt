<!--
  A quiet, clickable status line (the run status at the foot of the sidebar): an icon or a
  navy spinner, one short text (it wraps to a second line rather than being cut off) and,
  while something runs, a slim navy meter below. Hover washes it and turns the icon navy;
  a new text cross-fades in (100 ms). Collapsed (icon rail) only the icon stays; the text
  moves into the tooltip, right of the icon like the rail's. A failure keeps its danger tone on hover.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { fade } from '$lib/motion/transitions';
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
  use:tooltip={collapsed ? { text, placement: 'right' } : null}
  onclick={() => onclick()}
>
  <span class="line">
    <span class="glyph">
      {#if busy}<Spinner size="sm" label={null} progress />{:else}<Icon
          name={icon}
          size="sm"
        />{/if}
    </span>
    {#if !collapsed}
      {#key text}<span class="text" in:fade>{text}</span>{/key}
    {/if}
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
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  .status:hover {
    background-color: var(--surface-hover);
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  .status:active:hover {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
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
    align-items: flex-start;
    gap: var(--space-8);
    min-width: 0;
  }

  /* The glyph sits on the axis of the first text line; it turns navy on hover (a failure
     keeps its red). */
  .glyph {
    display: inline-flex;
    flex: none;
    align-items: center;
    height: var(--leading-sm);
    transition: color var(--dur-base) var(--ease-standard);
  }

  .neutral:hover .glyph {
    color: var(--icon-accent);
    transition-duration: var(--dur-hover);
  }

  .text {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
</style>
