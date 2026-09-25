<!--
  The handle between two columns (the job list and the reader, nowhere else): it takes no
  room of its own, an 8 px strip over the columns' border catches the pointer. Drag it with
  the left button to resize the column before it between `min` and `max`; a double click
  sets the width back to the first width. The limits and the first width follow the window
  and the sidebar (`splitLimits`): the list keeps at least 320 px, the reader at least
  440 px, and the list takes at most 60 % of the content. The width is kept per user
  (`storageKey`, in this browser profile; a store that cannot be read or written simply
  keeps the first width), and a kept width that does not fit is shown at the limit and comes
  back once there is room again. The col-resize cursor and, on hover or while dragging, a
  calm grip in the middle (like the Claude app's: no line along the border) show that it
  moves; after the usual delay
  the tooltip says so ("Breite ändern") over what a double click does. No keyboard: the
  columns are not a document to navigate.
-->
<script lang="ts" module>
  import { tokenPx } from '$lib/tokens';

  /** The limits of a list column beside a reader in a content of `width` px. */
  export interface SplitLimits {
    min: number;
    max: number;
    /** The first width (and the one a double click brings back). */
    initial: number;
  }

  /** The first width takes this share of the content (at most --list-first-max). */
  const FIRST_SHARE = 0.4;
  /** The list never takes more than this share of the content. */
  const MAX_SHARE = 0.6;

  export function splitLimits(width: number): SplitLimits {
    const min = tokenPx('--list-min');
    const max = Math.max(
      min,
      Math.min(width - tokenPx('--reader-min'), Math.round(width * MAX_SHARE)),
    );
    const first = Math.min(Math.round(width * FIRST_SHARE), tokenPx('--list-first-max'));
    return { min, max, initial: Math.max(min, Math.min(max, first)) };
  }
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';

  interface Props {
    /** The width of the column before the handle, in px (set by the handle). */
    size?: number | undefined;
    /** The width a double click restores (and the first width). */
    initial?: number;
    min?: number;
    max?: number;
    /** Where the width is kept (null: not kept). */
    storageKey?: string | null;
    /** What it resizes (accessible name). */
    label?: string;
    testid?: string | null;
  }

  let {
    size = $bindable(),
    initial = tokenPx('--list-min'),
    min = tokenPx('--list-min'),
    max = tokenPx('--list-first-max'),
    storageKey = null,
    label,
    testid = null,
  }: Props = $props();

  const clamp = (value: number): number => Math.round(Math.max(min, Math.min(max, value)));

  function stored(): number | null {
    if (storageKey === null) return null;
    try {
      const value = Number(localStorage.getItem(storageKey));
      return Number.isFinite(value) && value > 0 ? value : null;
    } catch {
      return null;
    }
  }

  function keep(value: number | null): void {
    if (storageKey === null) return;
    try {
      if (value === null) localStorage.removeItem(storageKey);
      else localStorage.setItem(storageKey, String(value));
    } catch {
      // Without a store the width lasts for this session only.
      return;
    }
  }

  /** The width the user chose (null: the first width); shown within the live limits. */
  let chosen = $state<number | null>(untrack(() => stored() ?? size ?? null));

  // The first frame already has the width; afterwards it follows the limits and the choice.
  size = untrack(() => clamp(chosen ?? initial));
  $effect.pre(() => {
    const width = clamp(chosen ?? initial);
    if (width !== untrack(() => size)) size = width;
  });

  let dragging = $state(false);
  let from = { x: 0, width: 0 };
  let frame = 0;
  let next: number | null = null;

  function start(event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    from = { x: event.clientX, width: size ?? clamp(initial) };
    next = null;
    dragging = true;
  }

  /** At most one resize per frame, however fast the pointer moves. */
  function move(event: PointerEvent): void {
    if (!dragging) return;
    next = clamp(from.width + event.clientX - from.x);
    if (frame !== 0) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (next !== null) chosen = next;
    });
  }

  function end(event: PointerEvent): void {
    if (!dragging) return;
    dragging = false;
    const handle = event.currentTarget as HTMLElement;
    if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId);
    if (frame !== 0) cancelAnimationFrame(frame);
    frame = 0;
    if (next === null) return;
    chosen = next;
    next = null;
    keep(chosen);
  }

  /** The second click of a double click sets the width back. */
  function reset(event: MouseEvent): void {
    if (event.button !== 0 || event.detail !== 2) return;
    chosen = null;
    next = null;
    keep(null);
  }
</script>

<div
  class="splitter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label ?? t.splitter.label}
  aria-valuenow={size}
  aria-valuemin={min}
  aria-valuemax={max}
  data-testid={testid ?? undefined}
>
  <!-- The strip the pointer takes; not in the Tab order (no keyboard). -->
  <button
    type="button"
    class="hit"
    tabindex="-1"
    aria-hidden="true"
    use:tooltip={{ text: t.splitter.tip, hint: t.splitter.reset, placement: 'right' }}
    onpointerdown={start}
    onpointermove={move}
    onpointerup={end}
    onpointercancel={end}
    onclick={reset}
  >
    <span class="grip"></span>
  </button>
</div>

<style>
  /* No room of its own: the strip lies over the border between the columns. */
  .splitter {
    position: relative;
    z-index: var(--z-raised);
    flex: none;
    width: 0;
  }

  .hit {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(-1 * var(--splitter-hit) / 2);
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--splitter-hit);
    cursor: col-resize;
    touch-action: none;
  }

  /* The grip: a calm pill centred on the border, darker while it is dragged. */
  .grip {
    flex: none;
    width: var(--grip-width);
    height: var(--grip-height);
    border-radius: var(--radius-full);
    background-color: var(--grip-rest);
    opacity: 0;
    transition:
      opacity var(--dur-base) var(--ease-standard),
      background-color var(--dur-base) var(--ease-standard);
  }

  .hit:hover .grip,
  .dragging .grip {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }

  .dragging .grip {
    background-color: var(--grip-drag);
  }
</style>
