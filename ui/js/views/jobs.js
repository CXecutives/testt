// Jobs: Liste links, Lesebereich rechts. Die Liste überfliegt man, den Text liest man –
// deshalb bekommt der Text den Platz.

import { api } from '../api.js';
import { startRun } from '../run.js';
import { locked, setSelection, state, subscribe } from '../store.js';
import {
  button, el, formatShort, icon, notice, plural, skeleton,
} from '../ui.js';

const n = {};
const rows = new Map();   // Schlüssel → { node, job }
const notices = [];
let jobs = [];
let detailSeq = 0;

const keyOf = (key) => `${key.portal}:${key.id}`;

export function build(page) {
  n.notices = el('div', { class: 'notices' });
  n.list = el('div', { class: 'list', id: 'list', role: 'listbox', 'aria-label': 'Jobs' });
  n.reader = el('div', { class: 'reader', id: 'reader' });
  n.split = el('div', { class: 'split', dataset: { pane: 'list' } }, n.list, n.reader);
  page.append(n.notices, n.split);

  subscribe('filter', () => load());
  subscribe('app', renderNotices);
  subscribe('busy', renderNotices);
  subscribe('selection', renderSelection);
  subscribe('jobs', () => load());
  subscribe('job', patch);
  renderReader();
  renderNotices();
}

/* ------------------------------------------------------------------ Liste */

export async function load() {
  const query = { latestRun: state.filter === 'new', search: state.search.trim() };
  let list;
  try {
    list = await api.listJobs(query);
  } catch (error) {
    notices.push({ level: 'error', text: `Liste nicht geladen: ${error.message}` });
    renderNotices();
    return;
  }
  jobs = list;
  render();
}

function render() {
  if (!jobs.length) {
    rows.clear();
    n.list.replaceChildren(el('div', { class: 'blank', text: emptyText() }));
    return;
  }
  const seen = new Set();
  const nodes = jobs.map((job, i) => {
    const key = keyOf(job.key);
    seen.add(key);
    const known = rows.get(key);
    if (known) {
      fill(known.node, job);
      known.job = job;
      return known.node;
    }
    const node = rowNode(job);
    // Nur beim ersten Erscheinen gestaffelt einblenden, und nur die ersten Zeilen –
    // bei 5000 Zeilen wäre eine Welle quer durch die Liste albern.
    if (i < 12) {
      node.classList.add('is-new');
      node.style.setProperty('--in-delay', `${i * 30}ms`);
    }
    rows.set(key, { node, job });
    return node;
  });
  for (const [key, entry] of rows) {
    if (!seen.has(key)) { entry.node.remove(); rows.delete(key); }
  }
  n.list.replaceChildren(...nodes);
  renderSelection();
}

function rowNode(job) {
  const node = el('button', {
    class: 'row', type: 'button', role: 'option', dataset: { key: keyOf(job.key) },
  },
  el('span', { class: 'row-title' }),
  el('span', { class: 'row-date' }),
  el('span', { class: 'row-meta' }));
  node.addEventListener('click', () => {
    node.blur();
    select(keyOf(job.key));
  });
  fill(node, job);
  return node;
}

/** Zeile an Ort und Stelle aktualisieren – nie ersetzen, sonst springt sie unter dem Zeiger weg. */
function fill(node, job) {
  const [title, date, meta] = node.children;
  title.textContent = job.title;
  date.textContent = formatShort(job.mailDate ?? job.firstSeenAt);
  const isNew = state.run.fresh.includes(keyOf(job.key));
  const flag = flagOf(job);
  meta.replaceChildren(...[
    isNew && el('span', { class: 'row-new' }),
    el('span', { class: 'row-where', text: [job.company, job.location].filter(Boolean).join(' · ') || '–' }),
    el('span', { class: 'row-flag', text: '·' }),
    el('span', { text: job.portalLabel }),
    flag && el('span', { class: `row-flag ${flag.level}`.trim(), text: `· ${flag.text}` }),
  ].filter(Boolean));
  node.title = job.title;
}

/** Ein Wort nur, wenn etwas vom Normalfall abweicht – sonst bliebe es Deko. */
function flagOf(job) {
  if (job.closed) return { level: '', text: 'geschlossen' };
  switch (job.status) {
    case 'missing':
    case 'failed':
      return { level: 'warn', text: 'ohne Details' };
    case 'gone':
      return { level: '', text: 'abgelaufen' };
    case 'unfetchable':
      return { level: '', text: 'nicht abrufbar' };
    default:
      return null;
  }
}

function emptyText() {
  if (state.search.trim()) return `Nichts gefunden für „${state.search.trim()}“.`;
  if (state.filter === 'all') return 'Noch keine Jobs gespeichert.';
  if (state.busy) return 'Noch keine neuen Jobs in diesem Lauf.';
  if (!state.app?.lastScanRun) return 'Noch keine Jobs – „Abrufen“ startet die Suche.';
  return 'Keine neuen Jobs im letzten Lauf. „Alle“ zeigt alle bisher gefundenen.';
}

function patch(job) {
  const entry = rows.get(keyOf(job.key));
  if (!entry) return;
  entry.job = job;
  fill(entry.node, job);
  if (state.selection === keyOf(job.key)) renderReader(job);
}

/* ------------------------------------------------------------------ Auswahl */

export function select(key) {
  if (state.selection === key) return;
  setSelection(key);
  if (matchMedia('(max-width: 859px)').matches) n.split.dataset.pane = 'reader';
  const entry = rows.get(key);
  renderReader(entry?.job ?? null);
  if (entry) void loadDetail(entry.job);
}

function renderSelection() {
  for (const [key, entry] of rows) {
    entry.node.setAttribute('aria-selected', String(key === state.selection));
  }
}

async function loadDetail(job) {
  const mine = ++detailSeq;
  const timer = setTimeout(() => {
    if (mine === detailSeq) n.reader.querySelector('.reader-body')?.replaceChildren(skeleton());
  }, 150);
  try {
    const detail = await api.jobDetail(job.key);
    if (mine !== detailSeq) return;
    renderReader(detail.job, detail.text);
  } catch (error) {
    if (mine !== detailSeq) return;
    renderReader(job, null, error.message);
  } finally {
    clearTimeout(timer);
  }
}

/* ------------------------------------------------------------------ Lesebereich */

const NO_TEXT = {
  missing: 'Noch keine Jobdetails.',
  failed: 'Jobdetails ließen sich nicht holen.',
  gone: 'Die Anzeige gibt es nicht mehr.',
  unfetchable: 'Nach mehreren Fehlversuchen aufgegeben.',
};

function renderReader(job = null, text = undefined, error = null) {
  if (!job) {
    n.reader.replaceChildren(el('div', { class: 'blank', text: 'Links einen Job wählen.' }));
    return;
  }
  const meta = [job.company, job.location, job.portalLabel, formatShort(job.mailDate ?? job.firstSeenAt)]
    .filter(Boolean).join(' · ');

  const actions = el('div', { class: 'reader-actions' },
    matchMedia('(max-width: 859px)').matches
      ? button({ label: 'Liste', icon: 'back', variant: 'ghost', onClick: () => { n.split.dataset.pane = 'list'; } })
      : null,
    button({
      label: 'Anzeige öffnen', variant: 'primary', icon: 'external',
      onAction: () => api.openTarget({ kind: 'jobUrl', key: job.key }).catch(() => {}),
    }),
    job.status === 'ok' ? null : button({
      label: 'Details holen',
      title: locked() ? 'Erst nach dem laufenden Vorgang möglich' : null,
      onAction: () => startRun('fetch', [job.key]),
    }));
  for (const btn of actions.querySelectorAll('.btn')) btn.disabled = locked() && btn.textContent !== 'Anzeige öffnen' && btn.textContent !== 'Liste';

  const body = el('div', { class: 'reader-body' });
  if (error) body.append(el('p', { class: 'reader-empty', text: error }));
  else if (text === undefined) body.append(skeleton());
  else if (text) body.append(el('pre', { class: 'reader-text', text }));
  else body.append(el('p', { class: 'reader-empty', text: NO_TEXT[job.status] ?? 'Kein Text vorhanden.' }));

  n.reader.replaceChildren(el('div', { class: 'reader-inner' },
    el('h1', { class: 'reader-title', text: job.title }),
    el('p', { class: 'reader-meta', text: meta }),
    actions,
    el('hr', { class: 'reader-rule' }),
    body));
}

/* ------------------------------------------------------------------ Hinweiszeilen */

/** Höchstens zwei – nach Rang, damit die wichtigste immer oben steht. */
function renderNotices() {
  const app = state.app;
  if (!app) return;
  const list = [];

  if (app.gmailError) {
    list.push({ level: 'error', text: app.gmailError, action: { label: 'Einstellungen', onClick: goSettings } });
  } else if (!app.gmailUser) {
    list.push({ level: 'warn', text: 'Kein Postfach verbunden.', action: { label: 'Einrichten', onClick: goSettings } });
  }

  const stopped = app.portals.find((p) => p.pausedUntil || p.nextFreeAt);
  if (stopped) {
    const until = formatShort(stopped.pausedUntil ?? stopped.nextFreeAt);
    const why = stopped.pausedUntil ? 'pausiert' : 'Obergrenze erreicht';
    list.push({ level: 'warn', text: `${stopped.label} ${why} – weiter ab ${until}.` });
  }

  const due = app.portals.reduce((sum, p) => sum + p.due, 0);
  if (due && !state.busy) {
    list.push({
      level: 'info',
      text: `${plural(due, 'Job', 'Jobs')} ohne Jobdetails.`,
      action: { label: 'Nachholen', onClick: () => startRun('fetch') },
    });
  }

  const empties = state.run.emptyAlerts.length || app.zeroPostingMails.length;
  if (empties) {
    list.push({ level: 'warn', text: `${plural(empties, 'Alert-Mail', 'Alert-Mails')} ohne erkannte Jobs.` });
  }

  const shown = [...notices, ...list].slice(0, 2);
  n.notices.replaceChildren(...shown.map((item) => notice(item).node));
}

let goSettings = () => {};

export function onSettings(handler) {
  goSettings = handler;
}
