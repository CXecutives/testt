// Input policy: the app behaves like a native app, not like a web page.
//
// This is the only file with key, context-menu, auxiliary-button, wheel and gesture
// listeners (eslint + core/tests/ui_contract.rs). Installed once in main.ts.
//
// - controls react to the left button only: the right button never presses, focuses or
//   selects anything. There is no browser context menu; the OS's own menu appears where
//   a native app has one: in a text field (Ausschneiden, Kopieren, Einfügen, Alles
//   auswählen, each enabled by the field's state) and on selected copyable text
//   (Kopieren). Everywhere else a right click does nothing.
// - the middle button scrolls: pressed over a scroll area it starts the autoscroll of the
//   OS (WebView2 on Windows; macOS has none); anywhere else it does nothing; a middle
//   click never activates anything (no auxclick), the back/forward buttons do nothing
// - a double click does nothing (in text that copies it selects a word, like everywhere)
// - no dragging of text, links or images
// - text is selectable only in fields and where a user would copy it (`data-copy`: the ad
//   text, job title and facts, profile values, paths); Ctrl/Cmd+C copies such a selection
// - keys like in a native window: Tab and Shift+Tab move the focus, Enter and Space press
//   the focused button, switch or radio. Inside a field every character the keyboard
//   layout types (AltGr on Windows, Option on macOS: @ is Option+L on a German Mac) and
//   the editing keys of the OS (word and line moves, delete word, Shift selection,
//   Ctrl/Cmd+C/V/X/A/Z, redo) work. Enter saves and Esc cancels a form or dialog.
//   Everything else, including every WebView shortcut (reload, find, print, zoom,
//   devtools, caret browsing, Alt+Arrow back/forward), is swallowed.
// - a modal dialog holds the focus: Tab cycles inside it, Esc cancels it wherever the
//   focus is.
// - OS window and menu functions stay: Alt+F4 and Cmd+Q/W/M/H/, (Settings), Cmd+Option+H.
// - no Ctrl/Cmd+wheel zoom (the wheel is watched only while Ctrl or Cmd is held, so plain
//   scrolling never waits for the page) and no pinch zoom
// - no hover flicker while a list scrolls (`:root[data-scrolling]`, see onScroll)

import type { Action } from 'svelte/action';
import { de } from '../i18n/de';
import { popupEditMenu, type EditEntry } from '../ipc/api';
import { keyConventions, type KeyConventions } from '../platform';
import { tokenMs } from '../tokens';

const FIELD = 'input, textarea, [contenteditable="true"], [contenteditable=""]';
/** Text a user would copy (selectable, Ctrl/Cmd+C). */
const COPY = '[data-copy]';
const DIALOG = 'dialog, [role="dialog"], [role="alertdialog"]';
/** An open modal dialog: it holds the focus. */
const MODAL = '[aria-modal="true"]';
const FORM = '[data-form-keys]';
/** Controls that Enter and Space press. */
const PRESSABLE = 'button, [role="button"], [role="switch"], [role="radio"]';
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
  const selection = getSelection();
  return selection !== null && !selection.isCollapsed && selection.toString().trim() !== '';
}

/** An element between `target` and the page that scrolls (the middle button scrolls it). */
function inScrollArea(target: EventTarget | null): boolean {
  for (let node = target instanceof Element ? target : null; node; node = node.parentElement) {
    const style = getComputedStyle(node);
    const scrollsY = /auto|scroll/.test(style.overflowY) && node.scrollHeight > node.clientHeight;
    const scrollsX = /auto|scroll/.test(style.overflowX) && node.scrollWidth > node.clientWidth;
    if (scrollsY || scrollsX) return true;
  }
  return false;
}

function isWindowShortcut(event: KeyboardEvent): boolean {
  if (event.altKey && event.key === 'F4') return true;
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

function allowedInField(event: KeyboardEvent): boolean {
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

function dispatchFormKey(event: KeyboardEvent, target: EventTarget | null = event.target): void {
  if (event.isComposing || hasModifier(event)) return;
  let handler: (() => void) | null = null;
  if (event.key === 'Escape') {
    handler = handlerFor(target, 'cancel');
  } else if (event.key === 'Enter') {
    // Enter on a button presses that button; in a text area it starts a new line.
    if (closest(target, 'textarea') !== null || closest(target, PRESSABLE) !== null) return;
    handler = handlerFor(target, 'save');
  }
  if (handler === null) return;
  event.preventDefault();
  handler();
}

const isFocusMove = (event: KeyboardEvent): boolean => event.key === 'Tab' && !hasModifier(event);

/** Enter and Space press the focused button, switch or radio (the engine clicks it). */
const pressesControl = (event: KeyboardEvent): boolean =>
  (event.key === 'Enter' || event.key === ' ') &&
  !hasModifier(event) &&
  closest(event.target, PRESSABLE) !== null;

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

function onKeyDown(event: KeyboardEvent): void {
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
    dispatchFormKey(event);
    return;
  }
  if (isFocusMove(event) || pressesControl(event)) return;
  if (closest(event.target, `${FORM}, ${DIALOG}`) !== null) dispatchFormKey(event);
  event.preventDefault();
}

const prevent = (event: Event): void => event.preventDefault();

/** The context menu of a text field, like the OS's own: what the field's state allows. */
function fieldMenu(field: HTMLInputElement | HTMLTextAreaElement): EditEntry[] {
  // A right click in a field that is not focused focuses it (the edit commands act on it).
  if (document.activeElement !== field) field.focus();
  const editable = !field.readOnly && !field.disabled;
  const hidden = field instanceof HTMLInputElement && field.type === 'password';
  const selected = (field.selectionStart ?? 0) !== (field.selectionEnd ?? 0);
  return [
    { command: 'Cut', text: de.edit.cut, enabled: editable && selected && !hidden },
    { command: 'Copy', text: de.edit.copy, enabled: selected && !hidden },
    { command: 'Paste', text: de.edit.paste, enabled: editable },
    { command: 'SelectAll', text: de.edit.selectAll, enabled: field.value !== '' },
  ];
}

/** Copyable text under the pointer that is part of the current selection. */
function selectedCopy(target: EventTarget | null): boolean {
  const copy = closest(target, COPY);
  const selection = getSelection();
  if (copy === null || selection === null || selection.isCollapsed) return false;
  return selection.toString().trim() !== '' && selection.containsNode(copy, true);
}

/** The right click: the OS's menu in fields and on selected copyable text, else nothing. */
function onContextMenu(event: MouseEvent): void {
  event.preventDefault();
  const field = closest(event.target, 'input, textarea');
  if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {
    void popupEditMenu(fieldMenu(field));
  } else if (selectedCopy(event.target)) {
    void popupEditMenu([{ command: 'Copy', text: de.edit.copy, enabled: true }]);
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
 * Hover stays still while a list scrolls: `:root[data-scrolling]` is set from the first
 * scroll event until --scroll-idle after the last one, and the rows' hover rules wait for
 * `:root:not([data-scrolling])` (a flip restyles only the rows, never their contents). A
 * passive listener: it never delays a scroll.
 */
let scrollIdle: ReturnType<typeof setTimeout> | undefined;
/** --scroll-idle, read once: reading a token inside the handler would force a style
 *  recalculation on every scroll event (right after the attribute changed). */
let scrollIdleMs: number | null = null;

function onScroll(): void {
  const root = document.documentElement;
  scrollIdleMs ??= tokenMs('--scroll-idle');
  if (root.dataset.scrolling === undefined) root.dataset.scrolling = '';
  clearTimeout(scrollIdle);
  scrollIdle = setTimeout(() => delete root.dataset.scrolling, scrollIdleMs);
}

/**
 * The middle button keeps its default over a scroll area (the autoscroll needs it), but a
 * press must not focus the control under the pointer: after the default action the focus
 * goes back to where it was.
 */
function keepFocus(): void {
  const before = document.activeElement;
  setTimeout(() => {
    const now = document.activeElement;
    if (now === before || !(now instanceof HTMLElement) || inField(now)) return;
    now.blur();
    if (before instanceof HTMLElement && before !== document.body) before.focus();
  }, 0);
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
      if (event.button === LEFT) {
        // A button inside a field (show password, clear) leaves the caret in the field.
        if (closest(event.target, KEEP_FOCUS) !== null) event.preventDefault();
        return;
      }
      // The middle button starts the autoscroll over a scroll area; nothing else gets it.
      if (event.button === MIDDLE && inScrollArea(event.target)) {
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
      if (!selectable(event.target)) event.preventDefault();
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
  document.addEventListener(
    'keyup',
    (event) => {
      if (!event.ctrlKey && !event.metaKey) guardZoom(false);
    },
    capture,
  );
  window.addEventListener('blur', () => guardZoom(false));
  document.addEventListener('scroll', onScroll, { capture: true, passive: true });
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
