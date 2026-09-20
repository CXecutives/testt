// Bausteine der Oberfläche. Jedes Bedienelement entsteht hier – die Ansichten bauen nie
// selbst ein <button> oder <input>. So kann es keine zweite Bauart desselben Knopfs geben
// (ein Vertragstest hält das fest).
//
// Inhalte werden nie als HTML eingesetzt: Titel und Firmen stammen aus fremden Mails.

export const $ = (selector, root = document) => root.querySelector(selector);
export const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

/** Element bauen: el('div', { class, text, onclick, … }, ...kinder). */
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

const NS = 'http://www.w3.org/2000/svg';

function svg(viewBox, className, ...paths) {
  const node = document.createElementNS(NS, 'svg');
  node.setAttribute('class', className);
  node.setAttribute('viewBox', viewBox);
  node.setAttribute('aria-hidden', 'true');
  for (const [tag, attrs] of paths) {
    const child = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) child.setAttribute(k, v);
    node.append(child);
  }
  return node;
}

/* ------------------------------------------------------------------ Symbole */

// Eine Familie, 24er-Raster, gleicher Strich – keine gemischten Herkünfte.
const PATHS = {
  search: 'M11 4.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM15.8 15.8 20 20',
  folder: 'M3.5 6.5h5l1.6 2h10.4v9a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5V6.5Z',
  gear: 'M12 9.2a2.8 2.8 0 1 0 0 5.6 2.8 2.8 0 0 0 0-5.6ZM12 3.5l1 2.1 2.3-.5 1 1.7-1.4 1.9 1.4 1.9-1 1.7-2.3-.5-1 2.1h-2l-1-2.1-2.3.5-1-1.7L7.1 12 5.7 10.1l1-1.7 2.3.5 1-2.1Z',
  external: 'M14 5h5v5M19 5l-8 8M17 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 5 18.5v-9A1.5 1.5 0 0 1 6.5 8H11',
  mail: 'M3.5 6.5h17v11a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1v-11ZM4 7l8 5.5L20 7',
  back: 'M14.5 5.5 8 12l6.5 6.5',
  down: 'M6.5 9.5 12 15l5.5-5.5',
  up: 'M6.5 14.5 12 9l5.5 5.5',
  refresh: 'M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5V10h-5.5',
  close: 'M6 6l12 12M18 6 6 18',
  check: 'M5 12.5l4.5 4.5L19 7.5',
};

/** Strich-Symbol als SVG-Knoten. */
export const icon = (name) => svg('0 0 24 24', 'icon', ['path', { d: PATHS[name] ?? PATHS.search }]);

/** App-Zeichen der Titelleiste – dasselbe Motiv wie die 16-px-Stufe von tools/icon.py. */
export const brandMark = () => svg('0 0 16 16', 'brand-mark',
  ['rect', { class: 'plate', x: 1, y: 1, width: 14, height: 14, rx: 3.3 }],
  ['path', { class: 'paper', d: 'M5 4h2l1 1h3a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1Z' }],
  ['path', { class: 'tick', d: 'M6.3 8.6 7.6 10.1 9.95 7.2', 'stroke-width': 1.7 }]);

/* ------------------------------------------------------------------ Zeigerzustand */

// Ist der Zeiger nicht nachweislich über dem Fenster (verlassen, minimiert, anderes Fenster
// aktiv), zeigt nichts mehr :hover oder „gedrückt“ – bis zur nächsten echten Bewegung.
// Das beseitigt hängende Zustände für alle Elemente auf einmal.
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

/* ------------------------------------------------------------------ Knöpfe */

/** Ein Flug je Knopf: weitere Klicks während der Arbeit verpuffen, der Kreisel kommt
 *  erst nach 150 ms (sonst blitzt er bei schnellen Antworten nur auf). */
export function action(button, fn) {
  let running = false;
  const run = async () => {
    if (running || button.disabled) return;
    running = true;
    const timer = setTimeout(() => markBusy(button, true), 150);
    const busy = async (promise) => promise;
    try {
      await fn(busy);
    } finally {
      clearTimeout(timer);
      markBusy(button, false);
      running = false;
    }
  };
  button.addEventListener('click', run);
  return run;
}

/** `aria-busy` mit echtem Wert – ein leeres Attribut träfe den CSS-Selektor nicht. */
export function markBusy(node, on) {
  if (on) node.setAttribute('aria-busy', 'true');
  else node.removeAttribute('aria-busy');
}

/**
 * Knopf. `variant` bestimmt die Art (sekundär voreingestellt), `tone: 'danger'` färbt.
 * `onAction` läuft unter dem Ein-Flug-Schutz, `onClick` ist für sofortige Wirkung.
 * Nach dem Klick gibt der Knopf den Fokus ab – sonst bliebe ein Ring stehen.
 */
export function button({ label, icon: name = null, variant = 'secondary', tone = null,
  title = null, onClick = null, onAction = null, id = null } = {}) {
  const classes = ['btn'];
  if (variant === 'primary') classes.push('primary');
  if (variant === 'ghost') classes.push('ghost');
  if (tone === 'danger') classes.push('danger');
  const node = el('button', { class: classes.join(' '), type: 'button', id, title },
    name && icon(name), label && el('span', { text: label }));
  node.addEventListener('click', () => node.blur());
  if (onClick) node.addEventListener('click', onClick);
  if (onAction) node.run = action(node, onAction);
  return node;
}

/** Nur ein Symbol; die Beschriftung lebt im Tooltip und für Hilfsmittel. */
export function iconButton({ icon: name, label, onClick = null, onAction = null, id = null } = {}) {
  const node = el('button', {
    class: 'btn ghost icon', type: 'button', id, title: label, 'aria-label': label,
  }, icon(name));
  node.addEventListener('click', () => node.blur());
  if (onClick) node.addEventListener('click', onClick);
  if (onAction) node.run = action(node, onAction);
  return node;
}

/** Fensterknopf (nur Windows) – Plattform-Chrom, kein Bedienelement der App. */
export function captionButton({ id, label, paths, close = false }) {
  const mark = svg('0 0 10 10', 'caption-glyph',
    ...paths.map((d) => ['path', { d, fill: 'none', stroke: 'currentcolor', 'stroke-width': 1 }]));
  return el('button', {
    class: `win-btn${close ? ' win-close' : ''}`, type: 'button', id,
    tabindex: '-1', title: label, 'aria-label': label,
  }, mark);
}

/* ------------------------------------------------------------------ Umschaltleiste */

/** Umschaltleiste mit gleitendem Daumen. Zähler sind optional und rechtsbündig tabellarisch. */
export function segmented({ options, value, onChange }) {
  const thumb = el('div', { class: 'seg-thumb' });
  const items = options.map((option) => el('button', {
    class: 'seg-item', type: 'button', 'aria-pressed': String(option.value === value),
    dataset: { value: option.value },
  }, el('span', { text: option.label }), option.count !== undefined && el('span', { class: 'seg-count' })));
  const node = el('div', { class: 'seg', role: 'group' }, thumb, ...items);
  let current = value;

  const place = () => {
    const active = items.find((item) => item.dataset.value === current) ?? items[0];
    if (!active || !active.offsetParent) return;
    thumb.style.width = `${active.offsetWidth}px`;
    thumb.style.transform = `translateX(${active.offsetLeft - 3}px)`;
  };
  const set = (next) => {
    current = next;
    for (const item of items) item.setAttribute('aria-pressed', String(item.dataset.value === next));
    place();
  };
  for (const item of items) {
    item.addEventListener('click', () => {
      item.blur();
      if (item.dataset.value === current) return;
      set(item.dataset.value);
      onChange(item.dataset.value);
    });
  }
  // Erst wenn das Element im Baum hängt, stimmen die Maße.
  requestAnimationFrame(place);
  addEventListener('resize', place);

  return {
    node,
    set,
    setCounts(counts) {
      for (const item of items) {
        const slot = item.querySelector('.seg-count');
        if (slot) slot.textContent = String(counts[item.dataset.value] ?? 0);
      }
      place();
    },
    setDisabled(off) {
      for (const item of items) item.disabled = off;
    },
  };
}

/* ------------------------------------------------------------------ Schalter */

export function toggle({ checked = false, label = '', onChange, title = null } = {}) {
  const input = el('input', { type: 'checkbox', checked: checked || null });
  const node = el('label', { class: 'switch', title },
    input, el('span', { class: 'switch-track' }), label && el('span', { text: label }));
  input.addEventListener('change', () => onChange(input.checked));
  return {
    node,
    set(on) { input.checked = on; },
    setDisabled(off) { input.disabled = off; },
  };
}

/* ------------------------------------------------------------------ Eingabefeld */

export function field({ id, label, type = 'text', placeholder = '', value = '', onInput, hint = '', compact = false } = {}) {
  const input = el('input', {
    class: 'input', id, type, placeholder, value, autocomplete: 'off', spellcheck: 'false',
  });
  const note = compact ? null : el('p', { class: 'field-hint', text: hint });
  const node = el('div', { class: 'field' },
    label && el('label', { class: 'field-label', for: id, text: label }), input, note);
  if (onInput) input.addEventListener('input', () => onInput(input.value));
  return {
    node,
    input,
    setHint(text, level = '') { if (!note) return; note.textContent = text; note.className = `field-hint ${level}`.trim(); },
    setDisabled(off) { input.disabled = off; },
  };
}

/* ------------------------------------------------------------------ Hinweiszeile */

/** Eine Zeile über der Liste: Punkt, Satz, höchstens eine Aktion. */
export function notice({ level = 'info', text = '', action: act = null } = {}) {
  const dot = el('span', { class: 'notice-dot' });
  const body = el('span', { class: 'notice-text', text });
  const node = el('div', { class: `notice ${level}`.trim() }, dot, body);
  let current = null;
  const update = (next) => {
    node.className = `notice ${next.level ?? 'info'}`.trim();
    body.textContent = next.text ?? '';
    const wanted = next.action ?? null;
    if (JSON.stringify(wanted?.label) !== JSON.stringify(current?.label)) {
      node.querySelector('.btn')?.remove();
      if (wanted) node.append(button({ label: wanted.label, variant: 'ghost', onClick: wanted.onClick }));
      current = wanted;
    } else if (wanted) {
      current = wanted;
    }
  };
  update({ level, text, action: act });
  return { node, update };
}

/** Zeile der Jobliste: zwei Zeilen Text, links ein Balken für die Auswahl. */
export function listRow({ key, onSelect }) {
  const node = el('button', { class: 'row', type: 'button', role: 'option', dataset: { key } },
    el('span', { class: 'row-title' }),
    el('span', { class: 'row-date' }),
    el('span', { class: 'row-meta' }));
  node.addEventListener('click', () => {
    node.blur();
    onSelect(key);
  });
  return node;
}

/* ------------------------------------------------------------------ Einstellungszeile */

export function settingRow({ label, hint = null, controls = [], stacked = false } = {}) {
  const hintNode = hint === null ? null : el('span', { class: 'setting-hint', text: hint });
  const node = el('div', { class: `setting${stacked ? ' stacked' : ''}` },
    el('div', { class: 'setting-text' }, el('span', { class: 'setting-label', text: label }), hintNode),
    el('div', { class: 'setting-controls' }, ...controls));
  return { node, setHint(text) { if (hintNode) hintNode.textContent = text; } };
}

/* ------------------------------------------------------------------ Schritte */

export function steps(items) {
  const node = el('div', { class: 'steps' });
  const render = (list) => node.replaceChildren(...list.map((step, i) => el('div',
    { class: `step${step.done ? ' done' : ''}` },
    el('span', { class: 'step-num' }, step.done ? icon('check') : String(i + 1)),
    el('span', { class: 'step-label', text: step.title }),
    step.action && button({ label: step.action.label, variant: step.action.primary ? 'primary' : 'secondary', onClick: step.action.onClick }))));
  render(items);
  return { node, render };
}

export const skeleton = () => el('div', { class: 'reader-skeleton' },
  el('i', { class: 'skeleton' }), el('i', { class: 'skeleton' }), el('i', { class: 'skeleton' }),
  el('i', { class: 'skeleton' }), el('i', { class: 'skeleton' }));

/* ------------------------------------------------------------------ Dialoge */

// Ein Dialog ist ein Stopp. Deshalb gibt es ihn nur für Rückfragen mit Folgen und für
// Fehler, die der Nutzer selbst ausgelöst hat – alles andere sagt die Statusleiste.
const GRACE = 300;   // Klicks direkt nach dem Öffnen (zweiter Klick eines Doppelklicks)
const SHIELD = 200;  // Sperre nach der Antwort – kein Durchklicken auf das, was darunter liegt

let openDialog = null;

export function dialog({ id = null, title, body = '', actions = [], busy = false, stale = null }) {
  const sheet = $('#sheet');
  closeDialog();
  const openedAt = performance.now();
  let settle = null;
  const done = new Promise((resolve) => { settle = resolve; });

  const foot = el('div', { class: 'dialog-foot' });
  const box = el('div', { class: `dialog${busy ? ' busy' : ''}`, role: busy ? 'alertdialog' : 'dialog', 'aria-busy': busy || null },
    el('h2', { class: 'dialog-title', text: title }),
    el('div', { class: 'dialog-body' }, busy && el('span', { class: 'spinner' }), el('span', { text: body })),
    actions.length ? foot : null);

  const finish = (value) => {
    if (openDialog?.box !== box) return;
    openDialog = null;
    sheet.replaceChildren();
    sheet.hidden = true;
    html.classList.add('is-shielded');
    setTimeout(() => html.classList.remove('is-shielded'), SHIELD);
    settle(value);
  };

  for (const act of actions) {
    foot.append(button({
      label: act.label,
      variant: act.variant ?? 'secondary',
      tone: act.tone ?? null,
      onClick: () => finish(act.value),
    }));
  }

  // Hintergrund schließt nur, wenn Drücken und Loslassen beide dort lagen und der Dialog
  // ungefährlich ist. Sonst reicht ein Ziehen aus dem Dialog heraus, um ihn zu verlieren.
  let pressedBackdrop = false;
  sheet.addEventListener('pointerdown', (event) => { pressedBackdrop = event.target === sheet; }, { once: true });
  sheet.onclick = (event) => {
    if (event.target !== sheet || !pressedBackdrop) return;
    if (performance.now() - openedAt < GRACE) return;
    if (actions.some((a) => a.value === null)) finish(null);
  };

  sheet.replaceChildren(box);
  sheet.hidden = false;
  openDialog = { id, box, finish, stale };
  return done;
}

/** Schließt den offenen Dialog (z. B. weil sein Grund entfallen ist). */
export function closeDialog(id = null) {
  if (!openDialog) return;
  if (id && openDialog.id !== id) return;
  openDialog.finish(null);
}

/** Ist der Grund des offenen Dialogs entfallen? Dann verschwindet er von selbst. */
export function pruneDialog() {
  if (openDialog?.stale?.()) openDialog.finish(null);
}

export const confirmDialog = (title, body, { yes = 'Ja', danger = false } = {}) =>
  dialog({
    title,
    body,
    actions: [
      { label: 'Abbrechen', value: null },
      { label: yes, value: true, variant: 'primary', tone: danger ? 'danger' : null },
    ],
  }).then(Boolean);

export const errorDialog = (title, body) =>
  dialog({ title, body, actions: [{ label: 'OK', value: true, variant: 'primary' }] });

export const blocker = (id, title, body) => dialog({ id, title, body, busy: true });

/* ------------------------------------------------------------------ Formate */

const TIME = new Intl.DateTimeFormat('de-DE', { dateStyle: 'short', timeStyle: 'short' });
const SHORT = new Intl.DateTimeFormat('de-DE', { day: '2-digit', month: '2-digit' });
const CLOCK = new Intl.DateTimeFormat('de-DE', { hour: '2-digit', minute: '2-digit', second: '2-digit' });

const date = (value) => (value ? new Date(value) : null);

export function formatTime(value) {
  const d = date(value);
  return d && !Number.isNaN(d.getTime()) ? TIME.format(d) : '';
}

/** Kurz für die Liste: heute die Uhrzeit, sonst Tag und Monat. */
export function formatShort(value) {
  const d = date(value);
  if (!d || Number.isNaN(d.getTime())) return '';
  const now = new Date();
  const sameDay = d.getDate() === now.getDate() && d.getMonth() === now.getMonth()
    && d.getFullYear() === now.getFullYear();
  return sameDay ? CLOCK.format(d).slice(0, 5) : SHORT.format(d);
}

export const formatClock = (d) => CLOCK.format(d);

export function formatBytes(bytes) {
  return bytes >= 1024 ? `${(bytes / 1024).toFixed(1).replace('.', ',')} KB` : `${bytes} Bytes`;
}

/** Anzahl mit passender Form – ein Satz sagt nie „1 Jobs“. */
export const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

/** Pfade in der Mitte kürzen: Anfang und Dateiname sind das Wichtige. */
export function midTruncate(text, max = 52) {
  if (!text || text.length <= max) return text ?? '';
  const keep = Math.floor((max - 1) / 2);
  return `${text.slice(0, keep)}…${text.slice(-keep)}`;
}
