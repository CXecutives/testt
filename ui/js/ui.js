// Bausteine der Oberfläche: Elemente, Symbole, Zeigerzustand, Meldungen, Dialoge, Knöpfe.
// Inhalte werden nie als HTML eingesetzt – Titel und Firmen stammen aus fremden Mails.

export const $ = (selector, root = document) => root.querySelector(selector);
export const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

/** Element bauen: el('button', { class, text, onclick, … }, ...kinder). */
export function el(tag, props = null, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props || {})) {
    if (value === null || value === undefined || value === false) continue;
    if (key === 'class') node.className = value;
    else if (key === 'text') node.textContent = value;
    else if (key === 'dataset') Object.assign(node.dataset, value);
    else if (key.startsWith('on') && typeof value === 'function') node.addEventListener(key.slice(2), value);
    else if (value === true) node.setAttribute(key, '');
    else node.setAttribute(key, String(value));
  }
  for (const child of children.flat()) {
    if (child === null || child === undefined || child === false) continue;
    node.append(child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return node;
}

/* ------------------------------------------------------------------ Symbole */

const PATHS = {
  info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM12 11v5M12 8h.01',
  ok: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM8.5 12.3l2.4 2.4 4.6-4.9',
  warn: 'M12 4 2.8 19.5h18.4L12 4ZM12 10v4M12 17h.01',
  error: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM9.5 9.5l5 5M14.5 9.5l-5 5',
  question: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM9.6 9.5a2.5 2.5 0 1 1 3.6 2.2c-.8.4-1.2 1-1.2 1.8M12 16.5h.01',
  check: 'M5 12.5l4.5 4.5L19 7.5',
  close: 'M6 6l12 12M18 6 6 18',
};

/** Strich-Symbol (24er-Raster) als SVG-Knoten. */
export function icon(name) {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('class', 'icon');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(ns, 'path');
  path.setAttribute('d', PATHS[name] ?? PATHS.info);
  svg.append(path);
  return svg;
}

/* ------------------------------------------------------------------ Zeigerzustand */

// Ist der Zeiger nicht nachweislich über dem Fenster (Fenster verlassen, minimiert,
// maximiert, anderes Fenster aktiv), zeigt nichts mehr :hover oder „gedrückt“ – bis zur
// nächsten echten Zeigerbewegung. Das verhindert hängende Zustände für alle Elemente.
const html = document.documentElement;

function pointerBack() {
  html.classList.remove('pointer-away');
  removeEventListener('pointermove', pointerBack, true);
  removeEventListener('pointerdown', pointerBack, true);
}

export function pointerAway() {
  html.classList.add('pointer-away');
  addEventListener('pointermove', pointerBack, { capture: true, passive: true });
  addEventListener('pointerdown', pointerBack, { capture: true, passive: true });
}

addEventListener('blur', pointerAway);
addEventListener('resize', pointerAway);
document.addEventListener('visibilitychange', () => {
  if (document.hidden) pointerAway();
});
html.addEventListener('pointerleave', pointerAway);

/** Ausblenden mit Übergang; ohne Übergang (reduzierte Bewegung) sofort weg. */
function leave(node) {
  node.classList.add('is-leaving');
  const remove = () => node.remove();
  node.addEventListener('transitionend', remove, { once: true });
  setTimeout(remove, 250);
}

/* ------------------------------------------------------------------ Meldungen */

const TOAST_MS = { info: 4500, ok: 5000, warn: 9000, error: 12000 };
const TOAST_MAX = 4;

// Solange ein Dialog offen ist, warten Meldungen (sonst lägen sie unter dem Dialog).
let held = [];

export function toast(text, level = 'info') {
  if (!$('#modals').hidden) {
    if (held.length < TOAST_MAX) held.push([text, level]);
    return;
  }
  const box = $('#toasts');
  let timer = 0;
  let started = 0;
  let remaining = TOAST_MS[level] ?? TOAST_MS.info;
  const close = () => {
    clearTimeout(timer);
    removeEventListener('focus', arm);
    leave(node);
  };
  // Die Zeit läuft nur, wenn man die Meldung sehen kann: Fenster aktiv, Zeiger nicht darauf.
  function arm() {
    if (timer || !node.isConnected) return;
    if (!document.hasFocus()) {
      addEventListener('focus', arm, { once: true });
      return;
    }
    started = performance.now();
    timer = setTimeout(close, remaining);
  }
  const pause = () => {
    if (!timer) return;
    clearTimeout(timer);
    timer = 0;
    remaining = Math.max(1500, remaining - (performance.now() - started));
  };
  const node = el('div', { class: `toast ${level}`, role: level === 'error' ? 'alert' : 'status', onpointerenter: pause, onpointerleave: arm },
    icon(level),
    el('p', { text }),
    el('button', { class: 'btn icon-only', type: 'button', title: 'Schließen', 'aria-label': 'Schließen', onclick: close }, icon('close')));
  const shown = [...box.children].filter((t) => !t.classList.contains('is-leaving'));
  if (shown.length >= TOAST_MAX) shown[0].remove();
  box.append(node);
  arm();
}

/* ------------------------------------------------------------------ Dialoge */

// Dialoge kommen nacheinander, nie übereinander.
let queue = Promise.resolve();
/** Der gerade gezeigte Dialog: { id, finish }. */
let active = null;
/** Ids wartender Dialoge – und die, die sich erledigt haben, bevor sie dran waren. */
const queued = new Set();
const dropped = new Set();

// Eingaben kurz nach dem Öffnen gelten nicht (zweiter Klick eines Doppelklicks); nach der
// Antwort bleibt die App noch kurz gesperrt (kein Durchklicken) – ein Folgedialog übernimmt
// dann nahtlos.
const GRACE_MS = 300;
const SHIELD_MS = 200;
let closeTimer = 0;
let returnFocus = null;

const setInert = (value) => {
  $('.app').inert = value;
  $('#toasts').inert = value;
};

/**
 * Dialog; buttons: [{ label, value, kind, primary, focus }] – `[]` ergibt einen Sperrdialog
 * ohne Knöpfe (nur per `closeModal`). Liefert den value, null bei Esc oder Klick daneben.
 * Mit `id` lässt er sich per `closeModal(id)` schließen, auch solange er noch wartet.
 * Liefert `stale()` true, wenn er an der Reihe ist, entfällt er (Ergebnis null).
 */
export function modal({ id = null, title, body, buttons, kind = 'info', icon: symbol = null, stale = null }) {
  if (id !== null) queued.add(id);
  const shown = queue.then(() => show({ id, title, body, buttons, kind, symbol, stale }));
  queue = shown.catch(() => {});
  return shown;
}

/** Schließt den Dialog `id` (ohne id: den gerade gezeigten) mit null. */
export function closeModal(id = null) {
  if (active && (id === null || active.id === id)) active.finish(null, false);
  else if (id !== null && queued.has(id)) dropped.add(id);
}

function closeRoot() {
  const root = $('#modals');
  root.hidden = true;
  root.classList.remove('is-leaving');
  root.replaceChildren();
  setInert(false);
  returnFocus?.focus?.({ preventScroll: true });
  returnFocus = null;
  const waiting = held;
  held = [];
  for (const [text, level] of waiting) toast(text, level);
}

function show({ id, title, body, buttons, kind, symbol, stale }) {
  return new Promise((resolve) => {
    queued.delete(id);
    if ((id !== null && dropped.delete(id)) || stale?.()) {
      resolve(null);
      return;
    }
    const root = $('#modals');
    clearTimeout(closeTimer);
    root.classList.remove('is-leaving');
    if (root.hidden) {
      returnFocus = document.activeElement;
      setInert(true);
      root.hidden = false;
    }
    const configs = buttons ?? [{ label: 'OK', value: true, primary: true }];
    const closable = configs.length > 0;
    const openedAt = performance.now();
    let done = false;
    let downOnBackdrop = false;

    const finish = (value, byUser = true) => {
      if (done || (byUser && performance.now() - openedAt < GRACE_MS)) return;
      done = true;
      active = null;
      document.removeEventListener('keydown', onKey, true);
      root.onpointerdown = null;
      root.onclick = null;
      root.classList.add('is-leaving');
      closeTimer = setTimeout(closeRoot, SHIELD_MS);
      resolve(value);
    };
    active = { id, finish };

    const buttonNodes = configs.map((cfg) => el('button', {
      class: `btn ${cfg.kind || (cfg.primary ? 'primary' : '')}`.trim(),
      type: 'button',
      text: cfg.label,
      onclick: () => finish(cfg.value),
    }));
    const onKey = (event) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        if (closable) finish(null);
      } else if (event.key === 'Tab') {
        // Der Fokus bleibt im Dialog.
        event.preventDefault();
        if (!buttonNodes.length) return;
        const at = buttonNodes.indexOf(document.activeElement);
        const next = at < 0 ? 0 : (at + (event.shiftKey ? -1 : 1) + buttonNodes.length) % buttonNodes.length;
        buttonNodes[next].focus();
      }
      // Enter und Leertaste wirken nativ auf den fokussierten Knopf – nie pauschal auf den primären.
    };
    const chip = symbol === 'spinner'
      ? el('div', { class: 'chip' }, el('span', { class: 'spinner' }))
      : el('div', { class: `chip ${kind}` }, icon(symbol ?? kind));
    const dialog = el('div', {
      class: `modal${closable ? '' : ' is-blocker'}`, role: closable ? 'dialog' : 'alertdialog', 'aria-modal': 'true',
      'aria-labelledby': 'modal-title', 'aria-describedby': 'modal-body', 'aria-busy': closable ? null : 'true',
    },
    el('div', { class: 'modal-head' }, chip, el('h2', { id: 'modal-title', text: title })),
    el('div', { class: 'modal-body', id: 'modal-body' }, String(body).split(/\n{2,}/).map((part) => el('p', { text: part }))),
    closable ? el('div', { class: 'modal-foot' }, buttonNodes) : null);
    root.replaceChildren(dialog);
    // Schließen über den Hintergrund nur, wenn Drücken und Loslassen dort lagen (sonst
    // schlösse eine im Dialog begonnene Textauswahl ihn).
    root.onpointerdown = (event) => {
      downOnBackdrop = event.target === root;
    };
    root.onclick = (event) => {
      if (closable && downOnBackdrop && event.target === root) finish(null);
      downOnBackdrop = false;
    };
    document.addEventListener('keydown', onKey, true);
    // Anfangsfokus: ausdrücklich gewünschter Knopf, bei Zerstörerischem der harmlose.
    const wanted = configs.findIndex((c) => c.focus);
    const danger = configs.some((c) => c.kind === 'danger');
    const start = wanted >= 0 ? wanted
      : danger ? configs.findIndex((c) => c.kind !== 'danger')
        : configs.findIndex((c) => c.primary);
    (buttonNodes[start] || buttonNodes[0] || dialog).focus?.({ preventScroll: true });
  });
}

export const infoBox = (title, body, buttons) => modal({ title, body, buttons, kind: 'info' });
export const warnBox = (title, body) => modal({ title, body, kind: 'warn' });
export const errorBox = (title, body) => modal({ title, body, kind: 'error' });

/** Rückfrage: true nur bei „yes“; „no“, Esc und Klick daneben ergeben false. */
export const confirmBox = (title, body, { yes = 'Ja', no = 'Abbrechen', danger = false } = {}) =>
  modal({
    title,
    body,
    kind: danger ? 'warn' : 'info',
    icon: 'question',
    buttons: [
      danger ? { label: yes, value: true, kind: 'danger' } : { label: yes, value: true, primary: true },
      { label: no, value: false },
    ],
  }).then((value) => value === true);

/** Sperrdialog („Wird beendet…“): bleibt, bis `closeModal(id)` ihn schließt. */
export const blocker = (id, title, body) => modal({ id, title, body, buttons: [], icon: 'spinner' });

/* ------------------------------------------------------------------ Knöpfe */

/** Wartet ein Knopf aufs Backend, zeigt er einen Kreisel statt der Beschriftung. */
export function markBusy(node, on) {
  if (on) node.setAttribute('aria-busy', 'true');
  else node.removeAttribute('aria-busy');
}

/**
 * Asynchrone Aktion eines Knopfs: nie zweimal gleichzeitig (weitere Klicks warten auf
 * denselben Lauf). `fn(busy)` markiert mit `busy(promise)` das eigentliche Warten aufs
 * Backend – dauert es länger als 150 ms, zeigt der Knopf einen Kreisel (Breite bleibt).
 * Liefert die Startfunktion (für Enter-Taste und Rückfragen).
 */
export function action(button, fn) {
  let flight = null;
  const busy = (promise) => {
    const timer = setTimeout(() => markBusy(button, true), 150);
    return Promise.resolve(promise).finally(() => {
      clearTimeout(timer);
      markBusy(button, false);
    });
  };
  const run = () => {
    flight ??= Promise.resolve().then(() => fn(busy)).finally(() => {
      flight = null;
    });
    return flight;
  };
  button.addEventListener('click', () => {
    run();
  });
  return run;
}

/* ------------------------------------------------------------------ Format */

const DATE = new Intl.DateTimeFormat('de-DE', {
  day: '2-digit', month: '2-digit', year: 'numeric', hour: '2-digit', minute: '2-digit',
  timeZone: 'Europe/Berlin',
});

const CLOCK = new Intl.DateTimeFormat('de-DE', {
  hour: '2-digit', minute: '2-digit', second: '2-digit', timeZone: 'Europe/Berlin',
});

/** Zeitstempel der App (RFC 3339) als „19.09.2026 08:15“ – wie im Backend und in Excel. */
export function formatTime(value) {
  if (!value) return '';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : DATE.format(date).replace(', ', ' ');
}

const SHORT = new Intl.DateTimeFormat('de-DE', {
  day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit', timeZone: 'Europe/Berlin',
});

/** Kurz für enge Spalten: „19.09. 08:15“ (das Jahr steht im Tooltip). */
export function formatShort(value) {
  if (!value) return '';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : SHORT.format(date).replace(', ', ' ');
}

/** Uhrzeit einer Verlaufszeile („08:15:42“). */
export const formatClock = (date) => CLOCK.format(date);

/** Dateigröße: ab 1024 Bytes in KB mit einer Stelle. */
export function formatBytes(bytes) {
  return bytes >= 1024 ? `${(bytes / 1024).toFixed(1).replace('.', ',')} KB` : `${bytes} Bytes`;
}

/** Anzahl mit passender Form – ein Satz sagt nie „1 Jobs“. */
export const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

/** Zeilen mit Aufzählungspunkt. */
export const bullets = (items) => items.map((item) => `• ${item}`).join('\n');
