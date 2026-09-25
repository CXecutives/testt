<!--
  A list row: leading, content, trailing, top-aligned (mail style: three lines of content,
  one row height; a row whose title needs a second line grows by that line), the one inner
  padding of the columns on the sides. A hairline under each row; a list whose rows reach
  past its column (for the wash) insets the line with `--row-rule-inset`, so it is as wide
  as every other hairline there. Hover washes the row (80 ms in, 150 ms out), a press
  darkens it (60 ms); rows never move or scale. The selected row takes a very light warm
  wash (one step deeper under the pointer, one more while pressed) and a coral bar on the
  left, inset by the row's padding, that fades in (150 ms) and out (100 ms); a row created
  as selected is simply there. A list that marks its open row with one bar of its own,
  which slides from row to row (the job list), turns the row's bar off (`bar={false}`): it
  goes at once, as the list's bar takes its place. While the window is inactive the
  selection turns grey (its ring track too), as in Mail and Explorer. While the list
  scrolls rows take no hover: a row
  rests (`data-rests`), and the rows the pointer passes during a scroll carry `data-still`
  (input.ts) until it is over, so only those rows restyle. A mark on :root or a property that
  inherits (pointer-events) would restyle every row twice per scroll, a long task with a few
  hundred rows. A list is one Tab stop: only its `tabbable` row takes Tab, the arrows move
  within. Under the keyboard focus ring the coral bar steps inside it (navy and coral touch,
  never blend).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    selected?: boolean;
    /** A selected row draws its own bar (false: the list's one sliding bar marks it). */
    bar?: boolean;
    /** Greyed out (excluded jobs behind the divider). */
    muted?: boolean;
    /** The click (its modifiers say whether it extends a selection). */
    onclick?: ((event: MouseEvent) => void) | null;
    /** Tab stops here (a list has one such row; the others are reached with the arrows). */
    tabbable?: boolean;
    testid?: string | null;
    leading?: Snippet | null;
    trailing?: Snippet | null;
    children: Snippet;
  }

  let {
    selected = false,
    bar = true,
    muted = false,
    onclick = null,
    tabbable = true,
    testid = null,
    leading = null,
    trailing = null,
    children,
  }: Props = $props();
</script>

<button
  type="button"
  class="row"
  class:selected
  class:bar
  class:muted
  aria-current={selected ? 'true' : undefined}
  tabindex={tabbable ? undefined : -1}
  data-rests=""
  data-testid={testid ?? undefined}
  onclick={(event) => onclick?.(event)}
>
  {#if leading}<span class="leading">{@render leading()}</span>{/if}
  <span class="content">{@render children()}</span>
  {#if trailing}<span class="trailing">{@render trailing()}</span>{/if}
</button>

<style>
  .row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-12);
    width: 100%;
    min-height: var(--row-height);
    overflow: hidden;
    padding: var(--space-12) var(--pane-padding);
    background-color: transparent;
    text-align: left;
    transition:
      background-color var(--dur-base) var(--ease-standard),
      opacity var(--dur-base) var(--ease-standard);
  }

  .row:hover:where(:not([data-still])) {
    background-color: var(--surface-hover);
    transition-duration: var(--dur-hover);
  }

  :global(:where(:root:not([data-aux-press]))) .row:active:hover {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
  }

  .selected {
    background-color: var(--surface-selected);

    /* The ring's track and a neutral badge stay visible on the warm wash. */
    --ring-track: var(--ring-track-selected);
    --badge-neutral-bg: var(--surface);
  }

  .selected:hover:where(:not([data-still])) {
    background-color: var(--surface-selected-hover);
  }

  /* Pressed, the warm wash deepens one more step (the grey press never covers it). */
  :global(:where(:root:not([data-aux-press]))) .selected:active:hover {
    background-color: var(--surface-selected-press);
    transition-duration: var(--dur-instant);
  }

  /* The selection bar on the left edge: always there, shown by opacity (it never
     changes shape). */
  .row::before {
    position: absolute;
    top: var(--row-bar-inset);
    bottom: var(--row-bar-inset);
    left: 0;
    width: var(--row-bar);
    border-radius: var(--radius-full);
    background-color: var(--selection-bar);
    content: '';
    opacity: 0;
    transition:
      opacity var(--dur-fast) var(--ease-in),
      background-color var(--dur-base) var(--ease-standard);
  }

  /* The hairline under the row (inset where the list says so). */
  .row::after {
    position: absolute;
    right: var(--row-rule-inset, 0);
    bottom: 0;
    left: var(--row-rule-inset, 0);
    height: var(--border-width);
    background-color: var(--border);
    content: '';
  }

  .selected.bar::before {
    opacity: 1;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out), var(--ease-standard);
  }

  /* The list's own bar takes over this row in the same frame: no fade under it. */
  .selected:not(.bar)::before {
    transition: none;
  }

  /* Like Mail and Explorer: the selection greys out while the window is in the back. */
  :global(:root[data-window='inactive']) .selected {
    background-color: var(--surface-selected-inactive);

    --ring-track: var(--ring-track-inactive);
  }

  :global(:root[data-window='inactive']) .selected::before {
    background-color: var(--text-subtle);
  }

  .row:focus-visible {
    box-shadow: var(--focus-ring-inset);
  }

  /* Inside the focus ring (as wide as --focus-ring-inset). */
  .row:focus-visible::before {
    left: var(--space-2);
  }

  .muted {
    opacity: var(--opacity-muted);
  }

  /* An excluded row brightens under the pointer: it invites reading, still grey. */
  .muted:hover:where(:not([data-still])) {
    opacity: var(--opacity-muted-hover);
  }

  .leading,
  .trailing {
    display: flex;
    flex: none;
  }

  .trailing {
    flex-direction: column;
    align-items: flex-end;
    gap: var(--space-4);
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
</style>
