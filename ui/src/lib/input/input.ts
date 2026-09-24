// Input policy: the app behaves like a native app, not like a web page.
//
// This is the only file with key, context-menu, auxiliary-button, wheel and gesture
// listeners (eslint + core/tests/ui_contract.rs). Installed once in main.ts.
//
// - controls react to the left button only: the right button never presses, focuses or
//   selects anything, and there is no context menu anywhere (fields included)
// - the middle button scrolls: pressed over a scroll area it starts the autoscroll of the
//   OS (WebView2 on Windows; macOS has none); anywhere else it does nothing; a middle
//   click never activates anything (no auxclick), the back/forward buttons do nothing
// - a double click does nothing (in text that copies it selects a word, like everywhere)
// - no dragging of text, links or images
// - text is selectable only in fields and where a user would copy it (`data-copy`: the ad
//   text, job title and facts, profile values, paths); Ctrl/Cmd+C copies such a selection
// - keys only inside fields and dialogs: Tab/Shift+Tab, Enter, Esc and the editing keys;
//   with Ctrl/Cmd only C/V/X/A/Z. Everything else, including every WebView shortcut
//   (reload, find, print, zoom, devtools), is swallowed.
// - OS window functions stay: Alt+F4 and Cmd+Q/W/M/H.
// - no Ctrl/Cmd+wheel zoom and no pinch zoom

import type { Action } from 'svelte/action';

const FIELD = 'input, textarea, [contenteditable="true"], [contenteditable=""]';
/** Text a user would copy (selectable, Ctrl/Cmd+C). */
const COPY = '[data-copy]';
const DIALOG = 'dialog, [role="dialog"], [role="alertdialog"]';
const FORM = '[data-form-keys]';

const CLIPBOARD_KEYS = new Set(['c', 'v', 'x', 'a', 'z']);
const LEFT = 0;
const MIDDLE = 1;
/** Back and forward (buttons 3 and 4). */
const BACK = 3;
const MAC_WINDOW_KEYS = new Set(['q', 'w', 'm', 'h']);
const DIALOG_KEYS = new Set(['Tab', 'Enter', 'Escape']);

/** The nearest match from `target` up (a text node - the target of selectstart - counts
 *  as its parent element). */
function closest(target: EventTarget | null, selector: string): Element | null {
  const element = target instanceof Text ? target.parentElement : target;
  return element instanceof Element ? element.closest(selector) : null;
}

const inField = (target: EventTarget | null): boolean => closest(target, FIELD) !== null;
const selectable = (target: EventTarget | null): boolean =>
  inField(target) || closest(target, COPY) !== null;

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
  return event.metaKey && !event.ctrlKey && MAC_WINDOW_KEYS.has(event.key.toLowerCase());
}

function allowedInField(event: KeyboardEvent): boolean {
  // AltGr (Ctrl+Alt on Windows) types characters such as @, € and { on German keyboards.
  const altGraph = event.getModifierState('AltGraph');
  const command = (event.ctrlKey || event.metaKey) && !altGraph;
  if (command) return CLIPBOARD_KEYS.has(event.key.toLowerCase()) && !event.altKey;
  if (event.altKey && !altGraph) return false;
  // Function keys (F1-F12) reach the WebView (reload, caret browsing, devtools).
  return !/^F\d{1,2}$/.test(event.key);
}

export interface FormKeyHandlers {
  /** Enter inside a single-line field. */
  save?: () => void;
  /** Esc anywhere inside the form. */
  cancel?: () => void;
}

const forms = new WeakMap<Element, FormKeyHandlers>();

/**
 * Enter = save, Esc = cancel for a form or dialog. No listener of its own: the one
 * keydown handler below dispatches to the nearest registered form.
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

function dispatchFormKey(event: KeyboardEvent): void {
  if (event.isComposing || (event.key !== 'Enter' && event.key !== 'Escape')) return;
  const form = closest(event.target, FORM);
  const handlers = form === null ? undefined : forms.get(form);
  if (handlers === undefined) return;
  if (event.key === 'Escape' && handlers.cancel) {
    event.preventDefault();
    handlers.cancel();
  } else if (event.key === 'Enter' && handlers.save && closest(event.target, 'textarea') === null) {
    event.preventDefault();
    handlers.save();
  }
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
  if (isWindowShortcut(event)) return;
  if (isCopy(event)) return;
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
  if (
    closest(event.target, DIALOG) !== null &&
    DIALOG_KEYS.has(event.key) &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey
  ) {
    dispatchFormKey(event);
    return;
  }
  event.preventDefault();
}

const prevent = (event: Event): void => event.preventDefault();

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

  document.addEventListener('contextmenu', prevent, capture);
  document.addEventListener(
    'mousedown',
    (event) => {
      if (event.button === LEFT) return;
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
    'wheel',
    (event) => {
      if (event.ctrlKey || event.metaKey) event.preventDefault();
    },
    { capture: true, passive: false },
  );
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
