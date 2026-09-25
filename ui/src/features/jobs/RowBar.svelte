<!--
  The job list's one selection bar: the coral bar on the left edge of the open job's row.
  Opening another job slides it there like the sidebar's pill (180 ms, emphasized), top and
  height together, so it keeps the row's inset on rows of 86 and 106 px; far jumps (Home,
  End) place it. It lies in the list's scroll content, so it scrolls with the rows, and it
  stays with its row whatever moves the rows: at once when rows arrive or fold away above it
  or its row grows, gliding along when the list re-sorts. It is simply there the first time:
  when the list is built or comes back, and after a reload, another filter or a search.
  While several rows are chosen each marks itself (the open one keeps this bar) and nothing
  slides; when the choice ends the bar is simply there. Opening a job while none is open
  fades it in (150 ms), closing fades it out (100 ms). A job that leaves the list (archive,
  trash) keeps it where its row was until the next job opens there. Inside the keyboard
  focus ring of its row it steps in, and it greys while the window is inactive, like a
  row's own bar (ListRow). Under reduced motion it never moves.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { px, setVars, type CssVars } from '$lib/actions/cssVars';
  import { barSlide, type BarBox } from '$lib/motion/transitions';
  import { tokenPx } from '$lib/tokens';

  interface Props {
    /** The rows (elements with `data-key`); their layout moves the bar. */
    rows: HTMLElement | null;
    /** The row the bar marks: the open job's, while it shows as selected. */
    open: string | null;
    /** That row is greyed out (an excluded job). */
    muted?: boolean;
    /** The listed rows: a row on the page that is not listed is on its way out. */
    listed: ReadonlySet<string>;
    /** Rows that fold away: their height leaves the list while they do. */
    folding: ReadonlySet<string>;
    /** Several rows are chosen (each marks itself). */
    several: boolean;
    /** Counts the lists: another list places the bar without motion. */
    generation: number;
  }

  let { rows, open, muted = false, listed, folding, several, generation }: Props = $props();

  let bar = $state<HTMLElement | null>(null);
  let shown = $state(false);
  /** This change of `shown` goes without a fade. */
  let snap = $state(false);
  let snapFrame = 0;

  /** The row the bar stands at (null: hidden). */
  let at: string | null = null;
  /** Where its style puts it. */
  let placed: BarBox | null = null;
  let vars: CssVars = {};
  /** The move under way: a slide to another row, or a glide with its row. */
  let motion: { animation: Animation; from: BarBox; to: BarBox } | null = null;
  /** The next placement jumps there: no slide, no fade. */
  let jump = true;
  /** The props as the last update saw them. */
  let seen: { open: string | null; several: boolean; generation: number } = {
    open: null,
    several: false,
    generation: Number.NaN,
  };
  /** The row found last (a lookup among thousands of rows only when it changed). */
  let found: HTMLElement | null = null;
  let inset = 0;

  function find(key: string): HTMLElement | null {
    if (found?.isConnected && found.dataset['key'] === key && rows?.contains(found)) return found;
    found = rows?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`) ?? null;
    return found;
  }

  /** The bar beside a row: its top in the list and its height, inset like the row's own. */
  function boxOf(item: HTMLElement): BarBox {
    const list = bar?.parentElement ?? null;
    let top = 0;
    for (let node: Element | null = item; node instanceof HTMLElement && node !== list;) {
      top += node.offsetTop;
      node = node.offsetParent;
    }
    inset ||= tokenPx('--row-bar-inset');
    return { top: top + inset, height: Math.max(0, item.offsetHeight - 2 * inset) };
  }

  /** Where the row will stand once the rows folding away above it are gone. */
  function settledBox(item: HTMLElement, live: BarBox): BarBox {
    let top = live.top;
    for (const key of folding) {
      if (listed.has(key)) continue;
      const other = rows?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`);
      if (!other || other === item) continue;
      if (other.compareDocumentPosition(item) & Node.DOCUMENT_POSITION_FOLLOWING) {
        top -= other.offsetHeight;
      }
    }
    return { top, height: live.height };
  }

  function inMotion(): boolean {
    return motion !== null && motion.animation.playState === 'running';
  }

  /** Where the bar is seen now (on its way, if it moves). */
  function current(): BarBox | null {
    if (motion === null || motion.animation.playState !== 'running') return placed;
    const { from, to, animation } = motion;
    const progress = animation.effect?.getComputedTiming().progress ?? 1;
    const along = (a: number, b: number): number => a + (b - a) * progress;
    return { top: along(from.top, to.top), height: along(from.height, to.height) };
  }

  function stop(): void {
    motion?.animation.cancel();
    motion = null;
  }

  function place(box: BarBox): void {
    if (bar === null) return;
    const next = { 'bar-top': px(box.top), 'bar-height': px(box.height) };
    setVars(bar, next, vars);
    vars = next;
    placed = box;
  }

  /** The style holds `to`; the move starts from `from`. */
  function slide(from: BarBox, to: BarBox, glide = false): void {
    stop();
    place(to);
    if (bar === null) return;
    const animation = barSlide(bar, from, to, glide);
    motion = animation === null ? null : { animation, from, to };
  }

  /** The next change of `shown` goes at once (for one frame). */
  function noFade(): void {
    snap = true;
    cancelAnimationFrame(snapFrame);
    snapFrame = requestAnimationFrame(() => (snap = false));
  }

  function reveal(fade: boolean): void {
    if (shown) return;
    if (!fade) noFade();
    shown = true;
  }

  function conceal(fade: boolean): void {
    at = null;
    stop();
    if (!shown) return;
    if (!fade) noFade();
    shown = false;
  }

  /** Brings the bar to the row it marks: after a change of the open job or of the list, and
   *  whenever the rows' layout changed (before it is drawn, so it never lags behind its
   *  row). */
  function update(): void {
    if (bar === null) return;
    const before = seen;
    seen = { open, several, generation };
    if (generation !== before.generation || (before.several && !several)) jump = true;
    // The user opened another job, or closed it.
    const changed = open !== before.open;
    if (open === null) {
      conceal(changed && !jump);
      return;
    }
    const item = find(open);
    if (item === null) {
      // Its row folded away already: the bar waits there for the next job.
      if (at === open && folding.has(open)) return;
      // Not mounted (further down than the rows shown so far) or not in this list.
      conceal(false);
      jump = true;
      return;
    }
    // The list is hidden (one column, reading): it is placed when the list shows again.
    if (item.offsetParent === null) {
      jump = true;
      return;
    }
    // Its row folds away (archived, deleted): the bar stays until the next job opens.
    if (!listed.has(open)) return;
    const live = boxOf(item);
    if (at !== open) {
      const to = settledBox(item, live);
      const from = shown ? current() : null;
      at = open;
      if (from !== null && changed && !jump && Math.abs(to.top - from.top) < innerHeight) {
        slide(from, to);
      } else {
        stop();
        place(to);
        reveal(changed && !jump);
      }
      jump = false;
      return;
    }
    if (inMotion() && motion !== null) {
      // The rows moved while it slides: on to where the row now ends up.
      const to = settledBox(item, live);
      const from = current();
      if (
        from !== null &&
        (px(to.top) !== px(motion.to.top) || px(to.height) !== px(motion.to.height))
      ) {
        slide(from, to);
      }
      return;
    }
    motion = null;
    place(live);
    reveal(false);
    jump = false;
  }

  /** The open row glides by `offset` to its new place (the list re-sorted): the bar glides
   *  along in the row's timing. */
  export function shift(offset: number): void {
    update();
    if (placed === null || !shown || at === null || inMotion()) return;
    slide({ top: placed.top + offset, height: placed.height }, placed, true);
  }

  /** Reports the rows' layout, right after each layout that changed their size. */
  let observer: ResizeObserver | null = null;

  $effect(() => {
    const root = rows;
    if (root === null) return;
    const watch = new ResizeObserver(() => update());
    watch.observe(root);
    observer = watch;
    return () => {
      watch.disconnect();
      if (observer === watch) observer = null;
    };
  });

  /**
   * An update after the next layout, before that frame is drawn: observed anew, the rows are
   * reported once more (in this very frame when they changed while it is being prepared, as
   * rows built in a frame callback do). A frame callback covers a list without a box (hidden
   * in one column), which reports nothing.
   */
  function soon(): () => void {
    if (observer !== null && rows !== null) {
      observer.unobserve(rows);
      observer.observe(rows);
    }
    const frame = requestAnimationFrame(update);
    return () => cancelAnimationFrame(frame);
  }

  // Another job opened in the same list: at once, so the bar sets off with the rows' wash.
  // Any other change of the list once its rows are laid out.
  $effect(() => {
    void open;
    void several;
    void generation;
    void listed;
    if (untrack(() => open !== seen.open && generation === seen.generation)) {
      untrack(update);
      return;
    }
    return untrack(soon);
  });

  $effect(() => () => {
    stop();
    cancelAnimationFrame(snapFrame);
  });
</script>

<span
  class="bar"
  class:shown
  class:snap
  class:muted
  aria-hidden="true"
  data-testid="row-bar"
  bind:this={bar}
></span>

<style>
  /* Placed by script (--bar-top, --bar-height), shown by opacity. */
  .bar {
    position: absolute;
    z-index: var(--z-raised);
    top: 0;
    left: 0;
    width: var(--row-bar);
    height: var(--bar-height, 0);
    border-radius: var(--radius-full);
    background-color: var(--selection-bar);
    opacity: 0;
    pointer-events: none;
    transform: translateY(var(--bar-top, 0));
    transition:
      opacity var(--dur-fast) var(--ease-in),
      background-color var(--dur-base) var(--ease-standard);
  }

  .shown {
    opacity: 1;
    transition-duration: var(--dur-base);
    transition-timing-function: var(--ease-out), var(--ease-standard);
  }

  /* On an excluded row it is as grey as the row. */
  .shown.muted {
    opacity: var(--opacity-muted);
  }

  .snap {
    transition: none;
  }

  /* Like Mail and Explorer: the selection greys out while the window is in the back. */
  :global(:root[data-window='inactive']) .bar {
    background-color: var(--text-subtle);
  }

  /* Inside the keyboard focus ring of its row (as wide as --focus-ring-inset). */
  :global(:has([data-open] :focus-visible)) > .bar {
    left: var(--space-2);
  }
</style>
