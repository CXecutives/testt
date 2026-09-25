<!--
  The right edge of the sidebar (the pattern of the column handle): it takes no room of its
  own, an 8 px strip over the sidebar's border catches the pointer, mostly on the sidebar's
  side so nothing of the sheet beside it is covered. On hover a 2 px navy line lies on the
  border with a small grip in its middle, and after the usual delay the tooltip says what a
  click does, "Seitenleiste einklappen" or "Seitenleiste ausklappen", over the key that does
  the same in a second, smaller line (Strg+B, ⌘B on macOS). A left click folds the sidebar
  to its icons or unfolds it (the parent keeps the choice). It starts below the toolbar row
  (macOS) and the sheet's rounded corner (Windows). Not in the Tab order: the key is the way
  of the keyboard, and the button tells assistive technology its state and its key.
-->
<script lang="ts">
  import { tooltip } from '$lib/actions/tooltip';
  import { t } from '$lib/i18n/t';
  import { ariaShortcut, commandKey } from '$lib/platform';

  interface Props {
    /** The sidebar shows its icons only. */
    collapsed: boolean;
    /** The id of the sidebar it folds. */
    controls?: string | null;
    testid?: string | null;
    ontoggle: () => void;
  }

  let { collapsed, controls = null, testid = null, ontoggle }: Props = $props();

  const label = $derived(collapsed ? t.shell.expandSidebar : t.shell.collapseSidebar);
</script>

<button
  type="button"
  class="edge"
  tabindex="-1"
  aria-label={label}
  aria-expanded={!collapsed}
  aria-controls={controls ?? undefined}
  aria-keyshortcuts={ariaShortcut('B')}
  data-testid={testid ?? undefined}
  use:tooltip={{ text: label, hint: t.shell.sidebarKey[commandKey()], placement: 'right' }}
  onclick={() => ontoggle()}
>
  <span class="line"></span>
  <span class="grip"></span>
</button>

<style>
  /* The strip ends one line width into the sheet: the line covers the sheet's hairline. */
  .edge {
    position: absolute;
    z-index: var(--z-raised);
    top: calc(var(--window-top) + var(--sheet-corner));
    right: calc(-1 * var(--splitter-line));
    bottom: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    width: var(--splitter-hit);
  }

  .line {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: var(--splitter-line);
    background-color: var(--active-edge);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  /* The grip: a small pill centred on the line. */
  .grip {
    width: var(--grip-width);
    height: var(--grip-height);
    margin-right: calc((var(--splitter-line) - var(--grip-width)) / 2);
    border-radius: var(--radius-full);
    background-color: var(--active-edge);
    opacity: 0;
    transition: opacity var(--dur-base) var(--ease-standard);
  }

  .edge:hover .line,
  .edge:hover .grip {
    opacity: 1;
    transition-duration: var(--dur-hover);
  }
</style>
