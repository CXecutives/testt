// Input policy: the app reacts to left click and hover - nothing else.
//
// This is the only file with key, context-menu, auxiliary-button, wheel and gesture
// listeners (eslint + core/tests/ui_contract.rs). Installed once in main.ts.
//
// - no context menu anywhere (fields included), no middle/right button, no autoscroll
// - double-click only in the title bar drag region (the OS maximizes the window there)
// - no dragging of text, links or images; no text selection outside fields
// - keys only inside fields and dialogs: Tab/Shift+Tab, Enter, Esc and the editing keys;
//   with Ctrl/Cmd only C/V/X/A/Z. Everything else, including every WebView shortcut
//   (reload, find, print, zoom, devtools), is swallowed.
// - OS window functions stay: Alt+F4 and Cmd+Q/W/M/H.
// - no Ctrl/Cmd+wheel zoom and no pinch zoom

import type { Action } from 'svelte/action';

const FIELD = 'input, textarea, [contenteditable="true"], [contenteditable=""]';
const DIALOG = 'dialog, [role="dialog"], [role="alertdialog"]';
const DRAG_REGION = '[data-tauri-drag-region]';
const FORM = '[data-form-keys]';

const CLIPBOARD_KEYS = new Set(['c', 'v', 'x', 'a', 'z']);
const MAC_WINDOW_KEYS = new Set(['q', 'w', 'm', 'h']);
const DIALOG_KEYS = new Set(['Tab', 'Enter', 'Escape']);

function closest(target: EventTarget | null, selector: string): Element | null {
  return target instanceof Element ? target.closest(selector) : null;
}

const inField = (target: EventTarget | null): boolean => closest(target, FIELD) !== null;

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

function onKeyDown(event: KeyboardEvent): void {
  if (isWindowShortcut(event)) return;
  if (inField(event.target)) {
    if (event.isComposing) return;
    if (!allowedInField(event)) {
      event.preventDefault();
      return;
    }
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

let installed = false;

export function installInput(): void {
  if (installed) return;
  installed = true;
  const capture = { capture: true } as const;

  document.addEventListener('contextmenu', prevent, capture);
  document.addEventListener(
    'mousedown',
    (event) => {
      if (event.button !== 0) event.preventDefault();
    },
    capture,
  );
  document.addEventListener(
    'auxclick',
    (event) => {
      if (event.button !== 0) event.preventDefault();
    },
    capture,
  );
  document.addEventListener(
    'dblclick',
    (event) => {
      if (closest(event.target, DRAG_REGION) === null) event.preventDefault();
    },
    capture,
  );
  document.addEventListener('dragstart', prevent, capture);
  document.addEventListener(
    'selectstart',
    (event) => {
      if (!inField(event.target)) event.preventDefault();
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
