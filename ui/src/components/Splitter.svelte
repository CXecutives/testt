<!--
  The handle between two columns (the job list and the reader, nowhere else): it takes no
  room of its own, an 8 px strip over the columns' border catches the pointer. Drag it with
  the left button to resize the column before it between `min` and `max`; a double click
  sets the width back. The width is kept per user (`storageKey`, in this browser profile;
  a store that cannot be read or written simply keeps the default). The col-resize cursor
  and, on hover or while dragging, a 2 px navy line show that it moves. No keyboard: the
  columns are not a document to navigate.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { t } from '$lib/i18n/t';
  import { tokenPx } from '$lib/tokens';

  interface Props {
    /** The width of the column before the handle, in px (unset: the kept or first width). */
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
    max = tokenPx('--list-max'),
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

  // The width to start from: the kept one, else the one given, else the first width.
  size = untrack(() => clamp(stored() ?? size ?? initial));

  let dragging = $state(false);
  let from = { x: 0, width: 0 };
  let frame = 0;
  let next = 0;

  function start(event: PointerEvent): void {
    if (event.button !== 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    from = { x: event.clientX, width: size ?? initial };
    next = 0;
    dragging = true;
  }

  /** At most one resize per frame, however fast the pointer moves. */
  function move(event: PointerEvent): void {
    if (!dragging) return;
    next = clamp(from.width + event.clientX - from.x);
    if (frame !== 0) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      size = next;
    });
  }

  function end(event: PointerEvent): void {
    if (!dragging) return;
    dragging = false;
    const handle = event.currentTarget as HTMLElement;
    if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId);
    if (frame !== 0) cancelAnimationFrame(frame);
    frame = 0;
    size = next === 0 ? size : next;
    keep(size ?? null);
  }

  /** The second click of a double click sets the width back. */
  function reset(event: MouseEvent): void {
    if (event.button !== 0 || event.detail !== 2) return;
    size = clamp(initial);
    next = 0;
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
    onpointerdown={start}
    onpointermove={move}
    onpointerup={end}
    onpointercancel={end}
    onclick={reset}
  >
    <span class="line"></span>
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
    justify-content: center;
    width: var(--splitter-hit);
    cursor: col-resize;
    touch-action: none;
  }

  .line {
    width: var(--splitter-line);
    height: 100%;
    background-color: var(--active-edge);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .hit:hover .line,
  .dragging .line {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }
</style>
