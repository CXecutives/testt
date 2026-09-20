// Statusleiste: der einzige Kanal, über den die App von sich aus etwas sagt.
// Ein Satz, ein Punkt, bei Bedarf Fortschritt und „Abbrechen“. Darüber fährt auf Wunsch
// die Aktivität aus – für die Frage „was hat es getan?“.

import { api } from '../api.js';
import { cancelRun, summaryLine } from '../run.js';
import { state, subscribe } from '../store.js';
import { button, el, formatClock, iconButton, plural } from '../ui.js';

const n = {};
let tick = 0;
let openPanel = false;

export function build(bar, panel) {
  n.bar = bar;
  n.dot = el('span', { class: 'status-dot' });
  n.spinner = el('span', { class: 'spinner', hidden: true });
  n.text = el('span', { class: 'status-text', id: 'status-text', role: 'status' });
  n.count = el('span', { class: 'status-count' });
  n.cancel = button({ label: 'Abbrechen', variant: 'ghost', tone: 'danger', onClick: cancelRun });
  n.cancel.hidden = true;
  n.toggle = iconButton({ icon: 'up', label: 'Aktivität', onClick: () => setPanel(!openPanel) });
  n.line = el('div', { class: 'status-line', id: 'status-line' });
  bar.append(n.line, n.dot, n.spinner, n.text, n.count, n.cancel, n.toggle);

  n.panel = panel;
  n.log = el('div', { class: 'panel-list', id: 'log' });
  panel.append(n.log);

  subscribe('run', render);
  subscribe('busy', render);
  subscribe('app', render);
  subscribe('log', renderLog);
  render();
  renderLog();
}

function setPanel(open) {
  openPanel = open;
  n.panel.hidden = !open;
  // Erst sichtbar, dann fahren – sonst gibt es keinen Übergang.
  requestAnimationFrame(() => n.panel.classList.toggle('is-open', open));
  n.toggle.replaceChildren(...iconButton({ icon: open ? 'down' : 'up', label: 'Aktivität' }).childNodes);
  n.toggle.title = open ? 'Aktivität schließen' : 'Aktivität';
  if (open) n.log.scrollTop = n.log.scrollHeight;
}

/** Ein Satz, weich gewechselt – harte Sprünge lesen sich wie Flackern. */
function setText(text) {
  if (n.text.textContent === text) return;
  n.text.classList.add('is-swapping');
  setTimeout(() => {
    n.text.textContent = text;
    n.text.classList.remove('is-swapping');
  }, 120);
}

function render() {
  clearTimeout(tick);
  const app = state.app;
  const run = state.run;
  const busy = state.busy || Boolean(state.session);

  n.spinner.hidden = !busy;
  n.dot.hidden = busy;
  n.cancel.hidden = !state.busy;
  n.cancel.disabled = state.cancelling;

  if (busy) {
    n.bar.dataset.level = '';
    setText(state.session ?? withCountdown(run.status, run.until));
    n.count.textContent = run.total ? `${run.done} / ${run.total}` : '';
    n.line.hidden = false;
    n.line.classList.toggle('indeterminate', !run.total);
    if (run.total) n.line.style.setProperty('--progress', String(run.done / run.total));
    if (run.until) tick = setTimeout(render, 1000 - (Date.now() % 1000) + 20);
    return;
  }

  n.line.hidden = true;
  n.count.textContent = '';
  n.bar.dataset.level = run.summary ? run.level : '';
  if (run.summary) setText(summaryLine(run.summary));
  else if (!app) setText('Wird geladen …');
  else if (!app.gmailUser) setText('Noch kein Postfach verbunden');
  else if (app.lastRun) setText(summaryLine(app.lastRun));
  else setText('Bereit');
  if (app?.dryRun) n.bar.dataset.level = 'warn';
}

/** „Pause vor dem nächsten Abruf (LinkedIn) – noch 12 s“ zählt sichtbar herunter. */
function withCountdown(text, until) {
  if (!until) return text;
  const left = Math.max(0, Math.round((new Date(until).getTime() - Date.now()) / 1000));
  return `${text} – noch ${plural(left, 'Sekunde', 'Sekunden')}`;
}

function renderLog() {
  const lines = state.log.map((entry) => el('div', { class: `log-line ${entry.level}`.trim() },
    el('time', { text: formatClock(new Date(entry.at)) }),
    el('span', { text: entry.text })));
  if (!lines.length) lines.push(el('div', { class: 'log-line', text: 'Noch nichts passiert.' }));
  const atEnd = n.log.scrollTop + n.log.clientHeight >= n.log.scrollHeight - 8;
  n.log.replaceChildren(...lines);
  if (atEnd) n.log.scrollTop = n.log.scrollHeight;
}

/** Kurzmeldung: drei Sekunden in der Statusleiste, dann wieder der normale Satz. */
export function flash(text) {
  setText(text);
  setTimeout(render, 3000);
}

export const openMail = (id) => api.openTarget({ kind: 'gmail', id }).catch(() => {});
