<!--
  The navigation of the sidebar: icon and label per view, no counts (the list says how many
  are new).
  The active entry sits on one white pill that slides to it (180 ms, emphasized; the
  sibling of the segmented thumb), its label ink and its icon coral. An idle entry washes
  on hover and its icon turns coral. Collapsed (icon rail) the labels move into tooltips
  right of the icons (never over the next entry). While the window is inactive the active
  label turns ink.
  An entry may carry sub-entries (Archiv, Papierkorb under Jobs), a group of its own:
  quieter (13 px, muted), indented under the parent's label, as high as the main entries so
  the one pill steps over them alike. Sub-entries are simply there when the nav mounts.
  With `fold` a small arrow at the end of the parent's row
  folds them away and back: it turns a quarter (180 ms), the sub-entries fade out where
  they are and the entries below then take their place (no height animation); unfolded
  the entries below make room and they fade in. Folded they stay in the document, hidden.
  While one of them is current they stay, and the arrow waits and says why. In the rail the
  sub-entries are smaller squares right under the parent's icon, with the arrow as a slim
  row between them and a hairline after them, so they read as its children and not as
  views of their own; the pill shrinks onto them, and their names are tooltips.
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface SideNavItem<Id extends string = string> {
    id: Id;
    label: string;
    icon: IconName;
    testid?: string;
    /** Quieter entries right under this one (the places of the Jobs view). */
    children?: readonly SideNavItem<Id>[];
  }

  /** The arrow that folds the sub-entries (of the first entry that has any) away. */
  export interface SideNavFold {
    /** The sub-entries show (while one of them is current they show anyway). */
    open: boolean;
    /** The arrow's name and tooltip while they show, and while they are hidden. */
    hide: string;
    show: string;
    /** Why the arrow waits while one of them is current. */
    locked: string;
    testid?: string;
    ontoggle: () => void;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { untrack } from 'svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import { cssVars } from '$lib/actions/cssVars';
  import { play } from '$lib/motion/motion';
  import { settled } from '$lib/motion/settled.svelte';
  import { fade } from '$lib/motion/transitions';
  import Icon from './Icon.svelte';

  interface Props {
    items: readonly SideNavItem<Id>[];
    /** `null`: no entry is current (a page that is none of the views, e.g. the setup). */
    active: Id | null;
    label: string;
    collapsed?: boolean;
    /** An arrow folds the sub-entries away (null: they always show). */
    fold?: SideNavFold | null;
    onselect: (id: Id) => void;
  }
  let { items, active, label, collapsed = false, fold = null, onselect }: Props = $props();
  const uid = $props.id();
  const groupId = `${uid}-group`;

  /** The entry whose sub-entries the arrow folds. */
  const parent = $derived(
    fold === null ? null : (items.find((item) => item.children?.length) ?? null),
  );
  /** One of its sub-entries is current: they stay in view, and the arrow waits. */
  const locked = $derived(parent?.children?.some((child) => child.id === active) ?? false);
  /** Whether the folded sub-entries show. */
  const shown = $derived(fold === null || fold.open || locked);
  /** Whether they are in the flow: from the moment they show until their fade-out ended.
   *  Out of it they stay in the document, hidden (no focus, no accessible name), so every
   *  entry of the nav always exists. */
  let inFlow = $state(untrack(() => shown));
  /** Counts the moves of the entries below the sub-entries (their pill jumps along). */
  let shifts = $state(0);
  /** The folded sub-entries (their fades play on it). */
  let group = $state<HTMLElement | null>(null);
  let fading: Animation | null = null;

  /** Every entry in the flow, in order: sub-entries right after their parent. */
  const entries = $derived(
    items.flatMap((item) => [
      { item, sub: false },
      ...(item === parent && !inFlow ? [] : (item.children ?? [])).map((child) => ({
        item: child,
        sub: true,
      })),
    ]),
  );
  const index = $derived(entries.findIndex((entry) => entry.item.id === active));
  /** Where the pill stands in the rail: the main entries, the sub-entries, the arrow and
   *  the hairlines that close a group of sub-entries above the current one, and whether it
   *  is a (smaller) sub-entry. */
  const pill = $derived.by(() => {
    let mains = 0;
    let subs = 0;
    let folds = 0;
    let ends = 0;
    for (let at = 0; at < index; at += 1) {
      const entry = entries[at];
      if (entry === undefined) break;
      if (!entry.sub) {
        mains += 1;
        if (entry.item === parent) folds += 1;
        continue;
      }
      subs += 1;
      if (entries[at + 1]?.sub !== true) ends += 1;
    }
    const sub = entries[index]?.sub ? 1 : 0;
    return { index: Math.max(0, index), mains, subs, folds, ends, sub };
  });
  /** The current entry comes after the folded sub-entries: it moves when they do. */
  const below = $derived(
    parent !== null && !locked && index > entries.findIndex((entry) => entry.item === parent),
  );
  const motion = settled();

  /** A sub-entry was current until now (they stayed only for it). */
  let wasLocked = untrack(() => locked);

  // Folding follows the arrow (and the current entry); at mount they are simply there.
  $effect(() => {
    const show = shown;
    const left = wasLocked && !locked;
    wasLocked = locked;
    // Leaving a sub-entry for an entry below them, they go at once: the pill takes the
    // shortest way to the entry's new place instead of sliding there and jumping after.
    untrack(() => (show ? unfold() : foldAway(left && below)));
  });

  /** The entries below make room at once; the sub-entries fade in where they now stand. */
  function unfold(): void {
    fading?.cancel();
    fading = null;
    if (inFlow) return;
    settle(true);
    const entrance = fadeGroup(0, 1);
    entrance?.addEventListener('finish', () => entrance.cancel());
  }

  /** They fade out where they are; then the entries below take their place at once. */
  function foldAway(now = false): void {
    if (!inFlow || fading !== null) return;
    const animation = now ? null : fadeGroup(1, 0);
    if (animation === null) {
      settle(false);
      return;
    }
    fading = animation;
    void animation.finished.then(
      () => {
        if (fading !== animation) return;
        fading = null;
        settle(false);
        // Hidden now: the held end of the fade goes with the same frame.
        animation.cancel();
      },
      () => undefined,
    );
  }

  /** A fade of the sub-entries (a cross-fade: it stays under reduced motion). */
  function fadeGroup(from: number, to: number): Animation | null {
    if (group === null) return null;
    return play(group, [{ opacity: from }, { opacity: to }], { duration: 'fast', crossfade: true });
  }

  /** In or out of the flow; the pill of an entry below jumps with it instead of sliding. */
  function settle(value: boolean): void {
    if (below) shifts += 1;
    inFlow = value;
  }

  function toggle(): void {
    if (!locked) fold?.ontoggle();
  }
</script>

{#snippet entry(item: SideNavItem<Id>, sub: boolean)}
  <button
    type="button"
    class="item"
    class:sub
    class:folds={item === parent}
    aria-current={item.id === active ? 'page' : undefined}
    aria-label={collapsed ? item.label : undefined}
    data-testid={item.testid}
    use:tooltip={collapsed ? { text: item.label, placement: 'right' } : null}
    onclick={() => onselect(item.id)}
  >
    <span class="glyph">
      <Icon name={item.icon} size={sub ? 'sm' : 'md'} />
    </span>
    {#if !collapsed}
      <span class="label" in:fade>{item.label}</span>
    {/if}
  </button>
{/snippet}

<nav class="nav" class:collapsed class:ready={motion.ready} aria-label={label} use:cssVars={pill}>
  <!-- Re-created when the rail flips or the entries below the sub-entries move with them,
       so it is placed without sliding. -->
  {#key `${collapsed}:${shifts}`}
    <span class="indicator" class:none={index < 0} aria-hidden="true"></span>
  {/key}
  {#each items as item (item.id)}
    {#if item === parent && fold !== null}
      <div class="parent">
        {@render entry(item, false)}
        <button
          type="button"
          class="fold"
          class:closed={!shown}
          aria-expanded={shown}
          aria-controls={groupId}
          aria-label={shown ? fold.hide : fold.show}
          aria-disabled={locked ? 'true' : undefined}
          tabindex={locked ? -1 : undefined}
          data-testid={fold.testid}
          use:tooltip={{
            text: locked ? fold.locked : shown ? fold.hide : fold.show,
            placement: 'right',
          }}
          onclick={toggle}
        >
          <span class="chevron"><Icon name="chevron-down" size={collapsed ? 'xs' : 'sm'} /></span>
        </button>
      </div>
      <div class="group" id={groupId} role="group" aria-label={item.label} hidden={!inFlow}>
        <div class="list" bind:this={group}>
          {#each item.children ?? [] as child (child.id)}{@render entry(child, true)}{/each}
        </div>
      </div>
    {:else}
      {@render entry(item, false)}
      {#if item.children?.length}
        <div class="group" role="group" aria-label={item.label}>
          <div class="list">
            {#each item.children as child (child.id)}{@render entry(child, true)}{/each}
          </div>
        </div>
      {/if}
    {/if}
  {/each}
</nav>

<style>
  .nav {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    isolation: isolate;
  }

  /* The one white pill behind the active entry. */
  .indicator {
    position: absolute;
    z-index: var(--z-below);
    top: 0;
    right: 0;
    left: 0;
    height: var(--control-md);
    border: var(--border-width) solid var(--nav-active-border);
    border-radius: var(--radius-md);
    background-color: var(--nav-active-bg);
    box-shadow: var(--sh-xs);
    transform: translateY(calc(var(--index) * var(--nav-step)));
    will-change: transform;
  }

  /* It slides only once the nav has been drawn (never when it mounts). */
  .ready .indicator {
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  .indicator.none {
    opacity: 0;
  }

  /* In the rail it steps over icons of two sizes and the arrow's row, and shrinks onto a
     sub-entry (centred in the column). */
  .collapsed .indicator {
    right: auto;
    width: var(--control-lg);
    height: var(--control-lg);
    transform: translate(
        calc(var(--sub) * var(--nav-sub-inset-rail)),
        calc(
          var(--mains) * var(--nav-step-rail) + var(--subs) * var(--nav-sub-step-rail) +
            var(--folds) * var(--nav-fold-rail) + var(--ends) * var(--nav-group-end-rail)
        )
      )
      scale(calc(1 + var(--sub) * (var(--nav-sub-scale-rail) - 1)));
    transform-origin: top left;
  }

  .item {
    display: flex;
    align-items: center;
    gap: var(--space-12);
    height: var(--control-md);
    padding: 0 var(--space-12);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font: var(--type-tab);
    transition:
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  /* The parent's row answers the pointer as a whole: over its arrow it keeps its wash, but
     not over an arrow that waits (nothing there reacts to a click). */
  .item:not([aria-current='page']):hover,
  .parent:hover:not(:has(.fold[aria-disabled='true']:hover)) > .item:not([aria-current='page']) {
    background-color: var(--surface-hover);
    color: var(--text);
    transition-duration: var(--dur-hover);
    --nav-glyph: var(--nav-active-icon);
  }

  :global(:where(:root:not([data-aux-press]))) .item:not([aria-current='page']):active:hover {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
  }

  .item[aria-current='page'] {
    color: var(--nav-active-fg);
    --nav-glyph: var(--nav-active-icon);
  }

  /* Like Mail and Explorer: the selection greys out while the window is in the back. */
  :global(:root[data-window='inactive']) .item[aria-current='page'] {
    color: var(--text);
    --nav-glyph: var(--text);
  }

  .item:focus-visible {
    box-shadow: var(--focus-ring);
  }

  .collapsed .item {
    justify-content: center;
    width: var(--control-lg);
    height: var(--control-lg);
    padding: 0;
  }

  .glyph {
    display: inline-flex;
    color: var(--nav-glyph, currentcolor);
    transition: color var(--dur-base) var(--ease-standard);
  }

  .label {
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* A sub-entry: quieter and indented under the parent's label (its icon where the
     parent's label starts), as high as a main entry. */
  .sub {
    padding-left: var(--nav-sub-indent);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  /* In the rail: a smaller square under the parent's icon. */
  .collapsed .sub {
    width: var(--nav-sub-rail);
    height: var(--nav-sub-rail);
    padding: 0;
    border-radius: var(--radius-sm);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .collapsed .list {
    align-items: center;
  }

  /* In the rail a hairline closes them: they belong to the icon above, not to the views
     below (it fades with them). */
  .collapsed .list::after {
    width: var(--space-24);
    height: var(--border-width);
    margin: var(--space-2) 0;
    background-color: var(--border);
    content: '';
  }

  /* The parent's row: its entry and, at the row's end, the arrow. */
  .parent {
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .folds {
    padding-right: var(--control-md);
  }

  .fold {
    position: absolute;
    top: calc((var(--control-md) - var(--nav-fold)) / 2);
    right: calc((var(--control-md) - var(--nav-fold)) / 2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--nav-fold);
    height: var(--nav-fold);
    border-radius: var(--radius-xs);
    color: var(--text-subtle);
    transition:
      background-color var(--dur-base) var(--ease-standard),
      color var(--dur-base) var(--ease-standard);
  }

  .fold:not([aria-disabled='true']):hover {
    background-color: var(--surface-hover);
    color: var(--text);
    transition-duration: var(--dur-hover);
  }

  :global(:where(:root:not([data-aux-press]))) .fold:not([aria-disabled='true']):active:hover {
    background-color: var(--surface-press);
    transition-duration: var(--dur-instant);
  }

  /* While a sub-entry is current the arrow waits (its tooltip says why). */
  .fold[aria-disabled='true'] {
    opacity: var(--opacity-disabled);
  }

  .fold:focus-visible {
    box-shadow: var(--focus-ring);
  }

  /* Down while the sub-entries show, a quarter turn to the right while they are hidden. */
  .chevron {
    display: inline-flex;
  }

  .closed .chevron {
    transform: rotate(calc(-1 * var(--turn-quarter)));
  }

  /* It turns only once the nav has been drawn (a kept state is simply there at start). */
  .ready .chevron {
    transition: transform var(--dur-slow) var(--ease-emphasized);
  }

  /* In the rail the arrow is a slim row under the parent's icon, the sub-entries under it. */
  .collapsed .parent {
    align-items: center;
  }

  .collapsed .folds {
    padding: 0;
  }

  .collapsed .fold {
    position: static;
    width: var(--control-lg);
    height: var(--nav-fold-rail);
  }
</style>
