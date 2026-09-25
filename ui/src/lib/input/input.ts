// Input policy: the app behaves like a native app, not like a web page.
//
// This is the only file with key, context-menu, auxiliary-button, wheel and gesture
// listeners (eslint + core/tests/ui_contract.rs). Installed once in main.ts.
//
// - controls react to the left button only: the right button never presses, focuses or
//   selects anything, and no control looks pressed under it (`data-aux-press`, see
//   auxPress). A left press beside a focused field ends its focus, also on a drag region
//   (`leaveField`). There is no browser context menu; the OS's own menu appears where
//   a native app has one: in a text field (Windows: Undo | Cut, Copy, Paste, Delete |
//   Select all; macOS: Cut, Copy, Paste | Select all; each enabled by the field's state)
//   and on selected copyable text (Copy). Everywhere else a right click does nothing.
// - the middle button scrolls: pressed over a scroll area it starts the autoscroll of the
//   OS (WebView2 on Windows; macOS has none); anywhere else it does nothing; a middle
//   click never activates anything (no auxclick), the back/forward buttons do nothing
// - a double click does nothing (in text that copies it selects a word, like everywhere)
// - no dragging of text, links or images
// - text is selectable only in fields and where a user would copy it (`data-copy`: the ad
//   text, job title and facts, profile values, paths); Ctrl/Cmd+C copies such a selection
// - keys like in a native window: Tab and Shift+Tab move the focus, Space presses the
//   focused button, switch or radio, Enter a button only. Inside a field every character the keyboard
//   layout types (AltGr on Windows, Option on macOS: @ is Option+L on a German Mac) and
//   the editing keys of the OS (word and line moves, delete word, Shift selection,
//   Ctrl/Cmd+C/V/X/A/Z, redo) work. Enter saves and Esc cancels a form or dialog.
// - a list with a reader (the Jobs view, `listKeys`) moves like a mail app: outside a field
//   ArrowUp/ArrowDown open the previous/next item, Home/End the first/last, Esc closes the
//   open item (in the search field Esc first clears the search), Space on the open item's
//   row pages through the reader (`reader`), and Ctrl+F (Cmd+F on macOS) goes to its search
//   field from anywhere.
//   Outside fields Ctrl+Z (Cmd+Z on macOS) takes back the last list action while it can
//   still be undone (`onUndo`), Ctrl+B (Cmd+B on macOS) folds the sidebar to its icons and
//   back (`onSidebarKey`; in a field it does nothing), and PageUp, PageDown, Space and
//   Shift+Space scroll the pane that has the focus (or the one clicked last) by a page,
//   like a native window. The macOS menu names Cmd+B too ("Seitenleiste ein-/ausblenden"):
//   the page takes the key first and prevents it, so WKWebView never hands it on to the
//   menu and the sidebar folds exactly once.
//   Everything else, including every WebView shortcut (reload, find, print, zoom,
//   devtools, caret browsing, Alt+Arrow back/forward), is swallowed.
// - a modal dialog holds the focus: Tab cycles inside it, Esc cancels it wherever the
//   focus is.
// - OS window and menu functions stay: Alt+F4 and Cmd+Q/W/M/H/, (Settings), Cmd+Option+H.
// - no Ctrl/Cmd+wheel zoom (the wheel is watched only while Ctrl or Cmd is held, so plain
//   scrolling never waits for the page) and no pinch zoom
// - no hover flicker while a list scrolls (`data-rests` and `data-still`, see onScroll)
// - Esc outside fields and dialogs clears what is selected (`escape`: the selection of
//   several jobs), like in a mail app

import type { Action } from 'svelte/action';
import { t } from '../i18n/t';
import { popupEditMenu, type EditEntry } from '../ipc/api';
import { fieldMenuUndoDelete, keyConventions, type KeyConventions } from '../platform';
import { tokenMs, tokenPx } from '../tokens';

const FIELD = 'input, textarea, [contenteditable="true"], [contenteditable=""]';
/** Text a user would copy (selectable, Ctrl/Cmd+C). */
const COPY = '[data-copy]';
const DIALOG = 'dialog, [role="dialog"], [role="alertdialog"]';
/** An open modal dialog: it holds the focus. */
const MODAL = '[aria-modal="true"]';
const FORM = '[data-form-keys]';
const LIST = '[data-list-keys]';
/** Controls that Space presses. */
const PRESSABLE = 'button, [role="button"], [role="switch"], [role="radio"]';
/** Controls that Enter presses: buttons only. A switch or a radio toggles with Space, like the
 *  native ones; Enter there goes on to the form (its default action). */
const ENTER_PRESSES = 'button:not([role="switch"], [role="radio"]), [role="button"]';
/** Buttons inside a field (show password, clear search): a press leaves the focus there. */
const KEEP_FOCUS = '[data-keep-focus]';
const FOCUSABLE = [
  'button:not([tabindex="-1"])',
  'input:not([tabindex="-1"])',
  'textarea:not([tabindex="-1"])',
  'a[href]:not([tabindex="-1"])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

const CLIPBOARD_KEYS = new Set(['c', 'v', 'x', 'a', 'z']);
/** Caret moves and deletes native fields do with Ctrl (Windows) or Cmd (macOS), with or
 *  without Shift: by word, to the line or text start and end, delete a word or line. */
const EDITING_KEYS = new Set([
  'ArrowLeft',
  'ArrowRight',
  'ArrowUp',
  'ArrowDown',
  'Home',
  'End',
  'Backspace',
  'Delete',
  'Insert',
]);
/** macOS text fields: Ctrl+A/E line start and end, B/F/N/P caret, D/H/K delete. */
const CONTROL_EDIT_KEYS = new Set(['a', 'e', 'b', 'f', 'n', 'p', 'd', 'h', 'k']);
const LEFT = 0;
const MIDDLE = 1;
/** Back and forward (buttons 3 and 4). */
const BACK = 3;
/** The Cmd shortcuts of the macOS menu (Quit, Close, Minimize, Hide, Settings). WKWebView
 *  hands a key equivalent to the menu only if the page lets it through. Matched by the
 *  character, like macOS matches key equivalents (Cmd+Q stays Q on AZERTY). */
const MAC_MENU_KEYS = new Set(['q', 'w', 'm', 'h', ',']);

/** The nearest match from `target` up (a text node - the target of selectstart - counts
 *  as its parent element). */
function closest(target: EventTarget | null, selector: string): Element | null {
  const element = target instanceof Text ? target.parentElement : target;
  return element instanceof Element ? element.closest(selector) : null;
}

const inField = (target: EventTarget | null): boolean => closest(target, FIELD) !== null;
const selectable = (target: EventTarget | null): boolean =>
  inField(target) || closest(target, COPY) !== null;

/** Ctrl, Alt or Cmd (Shift alone makes no shortcut). */
const hasModifier = (event: KeyboardEvent): boolean =>
  event.ctrlKey || event.altKey || event.metaKey;

/** Ctrl/Cmd+C over a selection of copyable text. */
function isCopy(event: KeyboardEvent): boolean {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || event.key.toLowerCase() !== 'c') {
    return false;
  }
  return hasSelection();
}

/** Selected text (outside fields only copyable text can be selected). */
function hasSelection(): boolean {
  const selection = getSelection();
  return selection !== null && !selection.isCollapsed && selection.toString().trim() !== '';
}

/** An element between `target` and the page that scrolls (the middle button scrolls it). A
 *  fixed layer (a dialog's backdrop, a toast, a tooltip) never scrolls with its DOM parent. */
function inScrollArea(target: EventTarget | null): boolean {
  for (let node = target instanceof Element ? target : null; node; node = node.parentElement) {
    const style = getComputedStyle(node);
    const scrollsY = /auto|scroll/.test(style.overflowY) && node.scrollHeight > node.clientHeight;
    const scrollsX = /auto|scroll/.test(style.overflowX) && node.scrollWidth > node.clientWidth;
    if (scrollsY || scrollsX) return true;
    if (style.position === 'fixed') return false;
  }
  return false;
}

/** The middle button scrolls here: over a scroll area, and while a modal dialog is open only
 *  inside it (the page behind it stays where it is). */
function middleScrolls(target: EventTarget | null): boolean {
  const modal = topModal();
  if (modal !== null && !(target instanceof Node && modal.contains(target))) return false;
  return inScrollArea(target);
}

function isWindowShortcut(event: KeyboardEvent): boolean {
  if (event.altKey && event.key === 'F4') return true;
  // Windows: Alt+Space opens the window's system menu (a cancelled key never reaches it).
  const plainAlt = event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
  if (plainAlt && event.code === 'Space' && keyConventions().systemMenuKey) return true;
  if (!event.metaKey || event.ctrlKey) return false;
  // Hide Others (Cmd+Option+H): Option changes the character, so the key itself counts.
  if (event.altKey) return event.code === 'KeyH';
  return MAC_MENU_KEYS.has(event.key.toLowerCase());
}

/** One printed character that no shortcut reports (a shortcut reports its plain letter or
 *  digit): @, €, {, |, ~, ą and so on. */
function isTypedCharacter(key: string): boolean {
  return [...key].length === 1 && !/^[a-z0-9]$/i.test(key);
}

/** A key typed with AltGr (Windows, where Ctrl+Alt works as AltGr too) or with Option
 *  (macOS: characters, dead keys, Option+Arrow and Option+Backspace by word). */
function typesWithAltGraph(event: KeyboardEvent, os: KeyConventions): boolean {
  if (event.getModifierState('AltGraph')) return true;
  if (os.optionTypes) return event.altKey && !event.ctrlKey && !event.metaKey;
  return event.ctrlKey && event.altKey && !event.metaKey && isTypedCharacter(event.key);
}

/** Shift+F10 alone: the context menu (Windows), which the engine turns into `contextmenu`. */
function isContextMenuKey(event: KeyboardEvent): boolean {
  return (
    event.key === 'F10' && event.shiftKey && !hasModifier(event) && keyConventions().contextMenuKey
  );
}

function allowedInField(event: KeyboardEvent): boolean {
  if (isContextMenuKey(event)) return true;
  // Function keys (F1-F12) reach the WebView (reload, caret browsing, devtools).
  if (/^F\d{1,2}$/.test(event.key)) return false;
  const os = keyConventions();
  if (typesWithAltGraph(event, os)) return true;
  // A plain Alt is the menu key on Windows, and Alt+Arrow navigates back and forward.
  if (event.altKey) return false;
  if (!event.ctrlKey && !event.metaKey) return true;
  if (event.ctrlKey && event.metaKey) return false;
  const key = event.key.toLowerCase();
  // Copy, paste, cut, select all, undo; with Shift, Z redoes.
  if (CLIPBOARD_KEYS.has(key)) return true;
  if (event[os.command] && EDITING_KEYS.has(event.key)) return true;
  if (os.redoWithY && event.ctrlKey && key === 'y') return true;
  return os.controlEdits && event.ctrlKey && CONTROL_EDIT_KEYS.has(key);
}

export interface FormKeyHandlers {
  /** Enter inside a single-line field (or on the dialog itself: its default button). */
  save?: () => void;
  /** Esc anywhere inside the form. */
  cancel?: () => void;
  /** Ctrl+S (Cmd+S on macOS) anywhere inside the form: saves a long form whose Enter
   *  already means something else (the next row of a list). */
  shortcut?: () => void;
}

const forms = new WeakMap<Element, FormKeyHandlers>();

/**
 * Enter = save, Esc = cancel for a form or dialog. No listener of its own: the one
 * keydown handler below dispatches to the nearest registered form that handles the key
 * (a search field that clears on Esc may sit inside a form that cancels on Esc).
 */
export const formKeys: Action<HTMLElement, FormKeyHandlers> = (node, handlers) => {
  forms.set(node, handlers);
  node.dataset.formKeys = '';
  return {
    update(next: FormKeyHandlers) {
      forms.set(node, next);
    },
    destroy() {
      forms.delete(node);
      delete node.dataset.formKeys;
    },
  };
};

function handlerFor(target: EventTarget | null, key: keyof FormKeyHandlers): (() => void) | null {
  for (let form = closest(target, FORM); form !== null; form = closest(form.parentElement, FORM)) {
    const handler = forms.get(form)?.[key];
    if (handler) return handler;
  }
  return null;
}

/** Ctrl+S or Cmd+S (the command key of the OS), without Alt or Shift. */
function isSaveShortcut(event: KeyboardEvent): boolean {
  return (
    event[keyConventions().command] &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 's'
  );
}

/** Enter and Esc for the nearest form that handles them; `true` if one did. */
function dispatchFormKey(event: KeyboardEvent, target: EventTarget | null = event.target): boolean {
  if (event.isComposing || hasModifier(event)) return false;
  let handler: (() => void) | null = null;
  if (event.key === 'Escape') {
    handler = handlerFor(target, 'cancel');
  } else if (event.key === 'Enter') {
    // Enter on a button presses that button; in a text area it starts a new line.
    if (closest(target, 'textarea') !== null || pressedByEnter(target)) {
      return false;
    }
    handler = handlerFor(target, 'save');
  }
  if (handler === null) return false;
  event.preventDefault();
  handler();
  return true;
}

export interface ListKeyHandlers {
  /** ArrowUp (-1) / ArrowDown (1): open the previous or next item. */
  step: (by: -1 | 1) => void;
  /** Home / End: open the first or the last item. */
  edge: (last: boolean) => void;
  /** Esc: close the open item. */
  close: () => void;
  /** Ctrl+F (Cmd+F on macOS): the search field. */
  find: () => void;
  /** The scroll area of the open item: Space and Shift+Space on the open item's row page
   *  through it, like in a mail app (pressing the row again would change nothing). */
  reader?: () => HTMLElement | null;
}

const lists = new Map<HTMLElement, ListKeyHandlers>();

/**
 * The keys of a list with a reader (the Jobs view): the one keydown handler below
 * dispatches to it while the focus is inside it or nowhere (a click on plain text leaves
 * the focus on the page).
 */
export const listKeys: Action<HTMLElement, ListKeyHandlers> = (node, handlers) => {
  lists.set(node, handlers);
  node.dataset.listKeys = '';
  return {
    update(next: ListKeyHandlers) {
      lists.set(node, next);
    },
    destroy() {
      lists.delete(node);
      delete node.dataset.listKeys;
    },
  };
};

/** A registered list the user sees (a view kept underneath another one is inert). */
function shownList(): ListKeyHandlers | null {
  for (const [node, handlers] of lists) {
    if (!node.isConnected || node.closest('[inert]') !== null) continue;
    if (node.getClientRects().length === 0 || getComputedStyle(node).visibility === 'hidden') {
      continue;
    }
    return handlers;
  }
  return null;
}

/** The list the key belongs to: the one around the focus, or the shown one without a focus. */
function listFor(target: EventTarget | null): ListKeyHandlers | null {
  const node = closest(target, LIST);
  if (node instanceof HTMLElement) return lists.get(node) ?? null;
  const nowhere = target === document.body || target === document.documentElement;
  return nowhere ? shownList() : null;
}

/** Ctrl+F or Cmd+F (the command key of the OS), without Alt or Shift. */
function isFindShortcut(event: KeyboardEvent): boolean {
  return (
    event[keyConventions().command] &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'f'
  );
}

/** Arrows, Home, End and Esc outside a field; `true` if a list took the key. */
function dispatchListKey(event: KeyboardEvent): boolean {
  if (event.isComposing || hasModifier(event) || event.shiftKey) return false;
  // The arrows of a radio group (the segments) are the group's.
  if (closest(event.target, '[role="radio"]') !== null && event.key.startsWith('Arrow')) {
    return false;
  }
  const list = listFor(event.target);
  if (list === null) return false;
  switch (event.key) {
    case 'ArrowUp':
      list.step(-1);
      return true;
    case 'ArrowDown':
      list.step(1);
      return true;
    case 'Home':
      list.edge(false);
      return true;
    case 'End':
      list.edge(true);
      return true;
    case 'Escape':
      list.close();
      return true;
    default:
      return false;
  }
}

/** The arrows inside a radio group (the segments): the previous or the next option takes
 *  the focus and is chosen, wrapping at the ends, like native radio buttons. The group is
 *  one Tab stop (only the chosen option has tabindex 0). `true` if the group took the key. */
function dispatchRadioKey(event: KeyboardEvent): boolean {
  if (hasModifier(event) || event.shiftKey) return false;
  const step =
    event.key === 'ArrowRight' || event.key === 'ArrowDown'
      ? 1
      : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
        ? -1
        : 0;
  const radio = closest(event.target, '[role="radio"]');
  const group = radio?.closest('[role="radiogroup"]') ?? null;
  if (step === 0 || radio === null || group === null) return false;
  const options = [...group.querySelectorAll<HTMLElement>('[role="radio"]')].filter(
    (node) => node.getAttribute('aria-disabled') !== 'true' && !node.matches(':disabled'),
  );
  const at = options.indexOf(radio as HTMLElement);
  const next = options[(at + step + options.length) % options.length];
  event.preventDefault();
  if (next === undefined || next === radio) return true;
  next.focus();
  next.click();
  return true;
}

const isFocusMove = (event: KeyboardEvent): boolean => event.key === 'Tab' && !hasModifier(event);

/** The focused control is a button that Enter presses (not a switch or a radio). */
function pressedByEnter(target: EventTarget | null): boolean {
  const control = closest(target, PRESSABLE);
  return control !== null && control.matches(ENTER_PRESSES);
}

/** Space presses the focused button, switch or radio, Enter only a button (the engine
 *  clicks it). */
const pressesControl = (event: KeyboardEvent): boolean =>
  !hasModifier(event) &&
  ((event.key === ' ' && closest(event.target, PRESSABLE) !== null) ||
    (event.key === 'Enter' && pressedByEnter(event.target)));

/** The open modal dialog on top, if any. */
function topModal(): HTMLElement | null {
  const open = document.querySelectorAll<HTMLElement>(MODAL);
  return open.item(open.length - 1);
}

/** Tab and Shift+Tab cycle through the controls of the modal and never leave it. */
function cycleFocus(modal: HTMLElement, back: boolean): void {
  const items = [...modal.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (node) => !node.matches(':disabled') && node.getClientRects().length > 0,
  );
  if (items.length === 0) {
    modal.focus();
    return;
  }
  const active = document.activeElement;
  const at = active instanceof HTMLElement ? items.indexOf(active) : -1;
  const step = back ? -1 : 1;
  const next =
    at === -1 ? (back ? items.length - 1 : 0) : (at + step + items.length) % items.length;
  items[next]?.focus();
}

export interface ChipKeyHandlers {
  /** Enter: turn the typed text into chips; `true` if there was text. */
  commit: () => boolean;
  /** Backspace in an empty field: remove the last chip; `true` if one went. */
  removeLast: () => boolean;
  /** Esc: drop the typed text; `true` if there was some. */
  clear: () => boolean;
}

const CHIPS = '[data-chip-keys]';
const chipFields = new WeakMap<Element, ChipKeyHandlers>();
/** A chip of a chip field whose chips can be edited, with its index (`data-chip`). */
const CHIP = '[data-chip]';
const EDITABLE_CHIPS = '[data-chip-edit]';
const chipEdits = new WeakMap<Element, (index: number) => void>();

/**
 * A chip field whose chips a double click takes back into its text for editing
 * (components/ChipInput.svelte): the chip carries its index in `data-chip`.
 */
export const chipEdit: Action<HTMLElement, (index: number) => void> = (node, edit) => {
  chipEdits.set(node, edit);
  node.dataset.chipEdit = '';
  return {
    update(next: (index: number) => void) {
      chipEdits.set(node, next);
    },
    destroy() {
      chipEdits.delete(node);
      delete node.dataset.chipEdit;
    },
  };
};

/** A double click with the left button on an editable chip edits it; `true` if it did. */
function editChip(event: MouseEvent): boolean {
  if (event.button !== LEFT) return false;
  const chip = closest(event.target, CHIP);
  const field = chip === null ? null : chip.closest(EDITABLE_CHIPS);
  const edit = field === null ? undefined : chipEdits.get(field);
  const index = Number(chip instanceof HTMLElement ? chip.dataset.chip : NaN);
  if (edit === undefined || !Number.isInteger(index)) return false;
  edit(index);
  return true;
}

/**
 * The keys of a chip field (components/ChipInput.svelte): Enter adds, Backspace in the empty
 * field removes the last chip, Esc drops the typed text. What a chip field does not use
 * goes on to the form (Enter on an empty chip field saves it).
 */
export const chipKeys: Action<HTMLElement, ChipKeyHandlers> = (node, handlers) => {
  chipFields.set(node, handlers);
  node.dataset.chipKeys = '';
  return {
    update(next: ChipKeyHandlers) {
      chipFields.set(node, next);
    },
    destroy() {
      chipFields.delete(node);
      delete node.dataset.chipKeys;
    },
  };
};

function dispatchChipKey(event: KeyboardEvent): boolean {
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return false;
  const field = closest(event.target, CHIPS);
  const handlers = field === null ? undefined : chipFields.get(field);
  if (handlers === undefined) return false;
  const handled =
    event.key === 'Enter'
      ? handlers.commit()
      : event.key === 'Backspace'
        ? handlers.removeLast()
        : event.key === 'Escape'
          ? handlers.clear()
          : false;
  if (handled) event.preventDefault();
  return handled;
}

/** The focus moves by the keyboard (Tab, the arrows, a key that opens something) until the
 *  next press of a mouse button. */
let keyboardFocus = false;

/**
 * A control the keyboard focuses stays clear of its scroll area's edges with its ring and a
 * gap (--space-8), below a sticky band too (the area's scroll-padding). The engines scroll a
 * focused control only until it touches the edge, and WebKit ignores scroll-margin there.
 */
function keepInView(node: HTMLElement): void {
  const pane = scrollAreaOf(node.parentElement);
  if (pane === null || !node.isConnected) return;
  const clear = tokenPx('--space-8');
  const style = getComputedStyle(pane);
  const view = pane.getBoundingClientRect();
  const top = view.top + pane.clientTop + (Number.parseFloat(style.scrollPaddingTop) || 0);
  const bottom =
    view.top +
    pane.clientTop +
    pane.clientHeight -
    (Number.parseFloat(style.scrollPaddingBottom) || 0);
  const box = node.getBoundingClientRect();
  const above = top - (box.top - clear);
  const below = box.bottom + clear - bottom;
  if (above > 0) pane.scrollTop -= above;
  else if (below > 0) pane.scrollTop += Math.min(below, box.top - clear - top);
}

function onFocusIn(event: FocusEvent): void {
  const node = event.target;
  if (!keyboardFocus || !(node instanceof HTMLElement)) return;
  // After the engine's own scroll into view.
  requestAnimationFrame(() => keepInView(node));
}

function onKeyDown(event: KeyboardEvent): void {
  keyboardFocus = true;
  if (event.ctrlKey || event.metaKey) guardZoom(true);
  if (isWindowShortcut(event)) return;
  const modal = topModal();
  if (modal !== null) {
    if (isFocusMove(event)) {
      event.preventDefault();
      cycleFocus(modal, event.shiftKey);
      return;
    }
    if (!(event.target instanceof Node) || !modal.contains(event.target)) {
      // The focus is behind the dialog: Esc and Enter still answer the dialog.
      dispatchFormKey(event, modal);
      event.preventDefault();
      return;
    }
  }
  if (isCopy(event)) return;
  // Shift+F10 on selected text opens its menu (Copy), like the Menu key.
  if (isContextMenuKey(event) && !inField(event.target) && hasSelection()) return;
  if (isFindShortcut(event)) {
    // Never the WebView's find bar; a list with a search field takes it.
    event.preventDefault();
    if (modal === null) shownList()?.find();
    return;
  }
  if (isSaveShortcut(event)) {
    // Never the WebView's "save page"; a form that saves this way gets it.
    event.preventDefault();
    handlerFor(event.target, 'shortcut')?.();
    return;
  }
  if (inField(event.target)) {
    if (event.isComposing) return;
    if (!allowedInField(event)) {
      event.preventDefault();
      return;
    }
    if (dispatchChipKey(event)) return;
    // Esc that no form takes (a search that is empty already) closes the list's open item.
    if (dispatchFormKey(event) || event.key !== 'Escape' || modal !== null) return;
    if (closest(event.target, LIST) !== null) dispatchListKey(event);
    return;
  }
  if (modal === null && pagesReader(event)) return;
  if (isFocusMove(event) || pressesControl(event) || dispatchRadioKey(event)) return;
  event.preventDefault();
  if (isUndo(event)) {
    if (modal === null) [...undos].reverse().some((undo) => undo());
    return;
  }
  if (isSidebarShortcut(event)) {
    if (modal === null) sidebarKey?.();
    return;
  }
  if (modal === null && scrollsPage(event)) return;
  if (closest(event.target, `${FORM}, ${DIALOG}`) !== null && dispatchFormKey(event)) return;
  if (event.key === 'Escape' && !hasModifier(event) && escapes.length > 0) {
    escapes.at(-1)?.();
    return;
  }
  if (modal === null) dispatchListKey(event);
}

/** The scroll area the user clicked in last (the page keys scroll it while the focus is
 *  nowhere, e.g. after a click on the ad's text). */
let lastPane: HTMLElement | null = null;
/** A page is this much of the pane (a line of the last page stays in view). */
const PAGE_SHARE = 0.9;

function scrollAreaOf(target: EventTarget | null): HTMLElement | null {
  for (let node = target instanceof Element ? target : null; node; node = node.parentElement) {
    if (!(node instanceof HTMLElement)) continue;
    const overflow = getComputedStyle(node).overflowY;
    if (/auto|scroll/.test(overflow) && node.scrollHeight > node.clientHeight) return node;
  }
  return null;
}

/** PageUp, PageDown, Space, Shift+Space: the focused (or last clicked) pane scrolls a page. */
function scrollsPage(event: KeyboardEvent): boolean {
  if (event.ctrlKey || event.altKey || event.metaKey) return false;
  const down = event.key === 'PageDown' || (event.key === ' ' && !event.shiftKey);
  const up = event.key === 'PageUp' || (event.key === ' ' && event.shiftKey);
  if (!down && !up) return false;
  const nowhere = event.target === document.body || event.target === document.documentElement;
  const pane = scrollAreaOf(event.target) ?? (nowhere && lastPane?.isConnected ? lastPane : null);
  if (pane === null) return false;
  scrollByPage(pane, down);
  return true;
}

function scrollByPage(pane: HTMLElement, down: boolean): void {
  const smooth = document.documentElement.dataset.motion !== 'reduce';
  pane.scrollBy({
    top: (down ? 1 : -1) * pane.clientHeight * PAGE_SHARE,
    behavior: smooth ? 'smooth' : 'auto',
  });
}

/** Space or Shift+Space on the open item's row: its reader scrolls a page. */
function pagesReader(event: KeyboardEvent): boolean {
  if (event.key !== ' ' || hasModifier(event)) return false;
  const row = closest(event.target, '[aria-current="true"]');
  if (row === null || closest(row, LIST) === null) return false;
  const pane = listFor(event.target)?.reader?.() ?? null;
  if (pane === null || !pane.isConnected) return false;
  event.preventDefault();
  scrollByPage(pane, !event.shiftKey);
  return true;
}

/** What Ctrl/Cmd+Z takes back outside fields (the last list action, like Mail). */
const undos: (() => boolean)[] = [];

/** `onUndo(handler)`: Ctrl+Z (Cmd+Z on macOS) outside fields and dialogs runs the newest
 *  handler that has something to undo. Returns the unsubscribe function. */
export function onUndo(handler: () => boolean): () => void {
  undos.push(handler);
  return () => {
    const at = undos.indexOf(handler);
    if (at !== -1) undos.splice(at, 1);
  };
}

/** Ctrl+Z or Cmd+Z (the command key of the OS), without Alt or Shift. */
function isUndo(event: KeyboardEvent): boolean {
  return (
    event[keyConventions().command] &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'z'
  );
}

/** What Ctrl+B (Cmd+B on macOS) does outside fields and dialogs: fold the sidebar. */
let sidebarKey: (() => void) | null = null;

/** `onSidebarKey(handler)`: Ctrl+B (Cmd+B on macOS) outside fields and dialogs runs it (the
 *  sidebar folds to its icons and back). Returns the unsubscribe function. */
export function onSidebarKey(handler: () => void): () => void {
  sidebarKey = handler;
  return () => {
    if (sidebarKey === handler) sidebarKey = null;
  };
}

/** Ctrl+B or Cmd+B (the command key of the OS alone), without Alt or Shift. */
function isSidebarShortcut(event: KeyboardEvent): boolean {
  return (
    event[keyConventions().command] &&
    !(event.ctrlKey && event.metaKey) &&
    !event.altKey &&
    !event.shiftKey &&
    event.key.toLowerCase() === 'b'
  );
}

/** What Esc clears outside fields and dialogs; the newest first. */
const escapes: (() => void)[] = [];

/**
 * `use:escape={clear}`: while the node is mounted, Esc outside fields and dialogs calls
 * `clear` (the selection bar: Esc clears the selection). No listener of its own.
 */
export const escape: Action<HTMLElement, () => void> = (_node, handler) => {
  let current = handler;
  const call = (): void => current();
  escapes.push(call);
  return {
    update(next: () => void) {
      current = next;
    },
    destroy() {
      escapes.splice(escapes.indexOf(call), 1);
    },
  };
};

const prevent = (event: Event): void => event.preventDefault();

/** The Delete entry of a field's menu: the selection goes, as one step of the field's undo. */
function deleteSelection(field: HTMLInputElement | HTMLTextAreaElement): void {
  field.focus();
  // The editing command keeps the step in the field's own undo (setRangeText would not).
  if (document.execCommand('delete')) return;
  field.setRangeText('', field.selectionStart ?? 0, field.selectionEnd ?? 0, 'end');
  field.dispatchEvent(new Event('input', { bubbles: true }));
}

const SEPARATOR: EditEntry = { command: 'Separator' };

/**
 * The context menu of a text field, like the OS's own, its entries enabled by the field's
 * state. Windows: Undo | Cut, Copy, Paste, Delete | Select all; macOS without undo and
 * delete (platform.ts).
 */
function fieldMenu(field: HTMLInputElement | HTMLTextAreaElement): EditEntry[] {
  // A right click in a field that is not focused focuses it (the edit commands act on it).
  if (document.activeElement !== field) field.focus();
  const editable = !field.readOnly && !field.disabled;
  const hidden = field instanceof HTMLInputElement && field.type === 'password';
  const selected = (field.selectionStart ?? 0) !== (field.selectionEnd ?? 0);
  const full = fieldMenuUndoDelete();
  const undo: EditEntry[] = full
    ? [
        // The engine keeps one undo history for the page (the entry sends Ctrl+Z).
        { command: 'Undo', text: t.edit.undo, enabled: editable && canUndo() },
        SEPARATOR,
      ]
    : [];
  const remove: EditEntry[] = full
    ? [
        {
          command: 'Delete',
          text: t.edit.delete,
          enabled: editable && selected,
          run: () => deleteSelection(field),
        },
      ]
    : [];
  return [
    ...undo,
    { command: 'Cut', text: t.edit.cut, enabled: editable && selected && !hidden },
    { command: 'Copy', text: t.edit.copy, enabled: selected && !hidden },
    { command: 'Paste', text: t.edit.paste, enabled: editable },
    ...remove,
    SEPARATOR,
    { command: 'SelectAll', text: t.edit.selectAll, enabled: field.value !== '' },
  ];
}

/** The page has an edit to take back (greys out the Undo entry otherwise, like the OS). */
function canUndo(): boolean {
  try {
    return document.queryCommandEnabled('undo');
  } catch {
    return true;
  }
}

/** Copyable text under the pointer that is part of the current selection. */
function selectedCopy(target: EventTarget | null): boolean {
  const copy = closest(target, COPY);
  const selection = getSelection();
  if (copy === null || selection === null || selection.isCollapsed) return false;
  return selection.toString().trim() !== '' && selection.containsNode(copy, true);
}

/** The right click: the OS's menu in fields and on selected copyable text, else nothing.
 *  From the keyboard (the Menu key, Shift+F10: no button) the menu opens where the engine
 *  puts the event, at the field or the selection, not at the pointer. */
function onContextMenu(event: MouseEvent): void {
  event.preventDefault();
  const at = event.button === -1 ? { x: event.clientX, y: event.clientY } : null;
  const field = closest(event.target, 'input, textarea');
  if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {
    void popupEditMenu(fieldMenu(field), at);
  } else if (selectedCopy(event.target)) {
    void popupEditMenu([{ command: 'Copy', text: t.edit.copy, enabled: true }], at);
  }
}

/**
 * Ctrl/Cmd+wheel would zoom. A wheel listener that may cancel is not passive, and a
 * page-wide one makes every scroll wait for the main thread, so it is attached only
 * while Ctrl or Cmd is held (WebView2 turns its zoom off natively as well).
 */
function blockZoom(event: WheelEvent): void {
  if (event.ctrlKey || event.metaKey) event.preventDefault();
  else guardZoom(false);
}

let zoomGuarded = false;

function guardZoom(on: boolean): void {
  if (on === zoomGuarded) return;
  zoomGuarded = on;
  if (on) document.addEventListener('wheel', blockZoom, { capture: true, passive: false });
  else document.removeEventListener('wheel', blockZoom, { capture: true });
}

/**
 * Hover stays still while a list scrolls: from the first scroll event until --scroll-idle
 * after the last one, an element marked `data-rests` (a list row) that is under the pointer,
 * or that the pointer meets while the content moves under it, carries `data-still`, and its
 * hover rules wait for `:not([data-still])`. Only the rows the pointer passes change: one
 * mark on :root restyled every row of a long list at the start and at the end of each
 * scroll (a long task with a few hundred rows). Passive listeners: they never delay a
 * scroll.
 */
let scrollIdle: ReturnType<typeof setTimeout> | undefined;
/** --scroll-idle, read once: reading a token inside the handler would force a style
 *  recalculation on every scroll event. */
let scrollIdleMs: number | null = null;
let scrolling = false;
/** The element under the pointer (the last `pointerover`); null once it left the window. */
let underPointer: Element | null = null;
/** The elements that rest until the scroll is over. */
const resting = new Set<HTMLElement>();
const RESTS = '[data-rests]';

/** Every element that rests while a list scrolls, from `target` outwards. */
function rest(target: Element | null): void {
  let node = target?.closest<HTMLElement>(RESTS) ?? null;
  while (node !== null) {
    if (!resting.has(node)) {
      node.dataset.still = '';
      resting.add(node);
    }
    node = node.parentElement?.closest<HTMLElement>(RESTS) ?? null;
  }
}

function scrollOver(): void {
  scrolling = false;
  for (const node of resting) delete node.dataset.still;
  resting.clear();
}

function onScroll(): void {
  scrollIdleMs ??= tokenMs('--scroll-idle');
  if (!scrolling) {
    scrolling = true;
    rest(underPointer);
  }
  clearTimeout(scrollIdle);
  scrollIdle = setTimeout(scrollOver, scrollIdleMs);
}

function onPointerOver(event: PointerEvent): void {
  underPointer = event.target instanceof Element ? event.target : null;
  if (scrolling) rest(underPointer);
}

function onPointerOut(event: PointerEvent): void {
  if (event.relatedTarget === null) underPointer = null;
}

/**
 * The middle button keeps its default over a scroll area (the autoscroll needs it), but a
 * press must not focus the control or the field under the pointer: after the default action
 * the focus goes back to where it was.
 */
function keepFocus(): void {
  const before = document.activeElement;
  setTimeout(() => {
    const now = document.activeElement;
    if (now === before || !(now instanceof HTMLElement)) return;
    now.blur();
    if (before instanceof HTMLElement && before !== document.body) before.focus();
  }, 0);
}

/**
 * The pressed look belongs to the left button. Both engines set `:active` (and Chromium
 * `:hover`) for any button in the hit test of the press, before a listener could cancel it,
 * and Chromium sets it again with the context menu after a right release. So while another
 * button is down, and until the pointer moves after it, :root carries `data-aux-press` and
 * every pressed rule waits for `:root:not([data-aux-press])` (core/tests/ui_contract.rs).
 */
function auxPress(on: boolean): void {
  const root = document.documentElement;
  if (on) root.dataset.auxPress = '';
  else if (root.dataset.auxPress !== undefined) delete root.dataset.auxPress;
}

/** Buttons other than the left one (the `buttons` bit mask without bit 0). */
const otherButtonsDown = (event: MouseEvent): boolean => (event.buttons & ~1) !== 0;

/** A press on a scroller's own scrollbar (Windows): it never takes the focus from a field. */
function onScrollbar(event: MouseEvent): boolean {
  const node = event.target;
  if (!(node instanceof HTMLElement) || node.clientWidth === 0) return false;
  const scrolls = node.scrollHeight > node.clientHeight || node.scrollWidth > node.clientWidth;
  // offsetX/Y count from the padding edge; the bar lies beyond the client box.
  return scrolls && (event.offsetX >= node.clientWidth || event.offsetY >= node.clientHeight);
}

/**
 * A left press beside the focused field ends its focus, like a click on the empty part of a
 * native window. The engine does that by itself only where the press keeps its default: a
 * drag region (the title bar, the toolbar row) cancels it. The field stays focused for a
 * press inside it or its box (the chips, the clear button), on a button that keeps the caret
 * (`data-keep-focus`), on its own label, and on a scrollbar.
 */
function leaveField(event: MouseEvent): void {
  const field = document.activeElement;
  if (!(field instanceof HTMLElement) || !inField(field) || inField(event.target)) return;
  const target = event.target instanceof Element ? event.target : null;
  if (target === null || closest(target, KEEP_FOCUS) !== null || onScrollbar(event)) return;
  const box = field instanceof HTMLTextAreaElement ? field : (field.parentElement ?? field);
  if (box.contains(target)) return;
  const label = target.closest('label');
  if (label !== null && label.control === field) return;
  field.blur();
}

let installed = false;

export function installInput(): void {
  if (installed) return;
  installed = true;
  const capture = { capture: true } as const;

  document.addEventListener('contextmenu', onContextMenu, capture);
  document.addEventListener(
    'mousedown',
    (event) => {
      keyboardFocus = false;
      if (event.button === LEFT) {
        if (!otherButtonsDown(event)) auxPress(false);
        lastPane = scrollAreaOf(event.target);
        // A button inside a field (show password, clear) leaves the caret in the field.
        if (closest(event.target, KEEP_FOCUS) !== null) event.preventDefault();
        else leaveField(event);
        return;
      }
      auxPress(true);
      // The middle button starts the autoscroll over a scroll area; nothing else gets it.
      if (event.button === MIDDLE && middleScrolls(event.target)) {
        keepFocus();
        return;
      }
      event.preventDefault();
    },
    capture,
  );
  // Back/forward buttons: Chromium navigates on their release.
  document.addEventListener(
    'mouseup',
    (event) => {
      if (event.button >= BACK) event.preventDefault();
    },
    capture,
  );
  // The pressed look comes back once no other button is down and the pointer moved (a right
  // release in Chromium sets :active once more with the context menu, before any move).
  document.addEventListener(
    'pointermove',
    (event) => {
      if (!otherButtonsDown(event)) auxPress(false);
    },
    { capture: true, passive: true },
  );
  document.addEventListener('pointercancel', () => auxPress(false), capture);
  document.addEventListener(
    'auxclick',
    (event) => {
      if (event.button !== LEFT) event.preventDefault();
    },
    capture,
  );
  // Some engines (WebKit) still send `click` for the middle button: it never reaches a
  // control.
  document.addEventListener(
    'click',
    (event) => {
      if (event.button === LEFT) return;
      event.preventDefault();
      event.stopImmediatePropagation();
    },
    capture,
  );
  document.addEventListener(
    'dblclick',
    (event) => {
      if (editChip(event) || !selectable(event.target)) event.preventDefault();
    },
    capture,
  );
  document.addEventListener('dragstart', prevent, capture);
  document.addEventListener(
    'selectstart',
    (event) => {
      if (!selectable(event.target)) event.preventDefault();
    },
    capture,
  );
  document.addEventListener('keydown', onKeyDown, capture);
  document.addEventListener('focusin', onFocusIn, { capture: true, passive: true });
  document.addEventListener(
    'keyup',
    (event) => {
      if (!event.ctrlKey && !event.metaKey) guardZoom(false);
    },
    capture,
  );
  window.addEventListener('blur', () => {
    guardZoom(false);
    auxPress(false);
  });
  document.addEventListener('scroll', onScroll, { capture: true, passive: true });
  document.addEventListener('pointerover', onPointerOver, { capture: true, passive: true });
  document.addEventListener('pointerout', onPointerOut, { capture: true, passive: true });
  // Safari/WKWebView pinch zoom.
  document.addEventListener('gesturestart', prevent, capture);
  document.addEventListener('gesturechange', prevent, capture);
}

/** Attributes every text field gets (applied by the TextField component). */
export const FIELD_ATTRIBUTES = {
  spellcheck: false,
  autocorrect: 'off',
  autocapitalize: 'off',
  autocomplete: 'off',
} as const;
