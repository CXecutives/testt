// Laufleiste unter der Kopfzeile, in jeder Ansicht: was die App gerade tut (mit Kreisel,
// Countdown und Fortschritt), sonst der letzte Stand – und „Abbrechen“, solange etwas läuft.

import { cancelRun } from '../run.js';
import { locked, state, subscribe } from '../store.js';
import { $, el, formatTime, markBusy, plural } from '../ui.js';

const n = {};
let tick = 0;

export function build() {
  n.bar = $('#runbar');
  n.icon = $('#run-icon');
  n.text = $('#run-status');
  n.count = $('#run-count');
  n.cancel = $('#run-cancel');
  n.line = $('#run-line');
  n.fill = $('.runbar-fill', n.line);
  n.dot = el('span', { class: 'dot' });
  n.spinner = el('span', { class: 'spinner' });
  n.cancel.addEventListener('click', cancelRun);
  subscribe('app', render);
  subscribe('busy', render);
  subscribe('run', render);
  render();
}

function render() {
  clearTimeout(tick);
  const busy = locked();
  const [text, level] = busy ? busyText() : idleText();
  if (n.text.textContent !== text) {
    n.text.textContent = text;
    n.text.title = text;
  }
  n.bar.dataset.level = level;
  n.dot.className = `dot ${level}`.trim();
  const iconNode = busy ? n.spinner : n.dot;
  if (n.icon.firstChild !== iconNode) n.icon.replaceChildren(iconNode);

  const progress = busy && state.busy ? state.run.progress : null;
  const total = progress?.total ?? 0;
  n.count.textContent = total ? countText(progress) : '';
  n.line.classList.toggle('is-active', busy);
  n.line.classList.toggle('is-indeterminate', busy && !total);
  const ratio = total ? Math.min(1, progress.done / total) : 0;
  n.fill.style.setProperty('--progress', String(ratio));
  if (busy && total) {
    n.line.setAttribute('aria-valuenow', String(Math.round(ratio * 100)));
    n.line.removeAttribute('aria-valuetext');
  } else {
    n.line.removeAttribute('aria-valuenow');
    n.line.setAttribute('aria-valuetext', busy ? text : 'Kein Lauf');
  }

  n.cancel.hidden = !busy;
  n.cancel.disabled = state.cancelling;
  markBusy(n.cancel, state.cancelling);
  n.cancel.title = state.session ? '' : 'Den Lauf nach dem aktuellen Schritt beenden';
}

function busyText() {
  if (state.cancelling) return ['Wird abgebrochen…', ''];
  if (state.session) return [state.session, ''];
  const run = state.run;
  let text = run.status || (run.kind === 'scan' ? 'Postfach wird abgerufen…' : 'Jobdetails werden vorbereitet…');
  const left = run.until ? Math.ceil((Date.parse(run.until) - Date.now()) / 1000) : 0;
  if (left > 0) {
    text += ` – noch ${left} s`;
    tick = setTimeout(render, 1000 - (Date.now() % 1000) + 20);
  }
  return [text, ''];
}

function countText({ step, done, total }) {
  if (step !== 'scan') return `${done} / ${plural(total, 'Jobdetail', 'Jobdetails')}`;
  return `${done} / ${plural(total, 'Mail', 'Mails')}`;
}

function idleText() {
  const app = state.app;
  if (!app) return ['Wird geladen …', ''];
  if (app.gmailError) return ['Gmail-Zugang unlesbar – siehe „System“.', 'warn'];
  if (state.run.summaryLine) return [state.run.summaryLine, state.run.summaryLevel];
  if (!app.dryRun && !app.gmailUser) return ['Noch kein Gmail-Zugang – bitte unter „System“ eintragen.', 'warn'];
  const last = app.lastRun;
  if (last) {
    const when = formatTime(last.finishedAt);
    if (last.outcome?.kind === 'failed') return [`Letzter Lauf ${when}: fehlgeschlagen – siehe Verlauf.`, 'error'];
    if (last.outcome?.kind === 'cancelled') return [`Letzter Lauf ${when}: abgebrochen, Teilergebnis gespeichert.`, 'warn'];
    if (last.scan) return [`Letzter Abruf ${when}: ${last.scan.new} neu, ${last.scan.knownBefore} bereits bekannt.`, 'ok'];
    return [`Letzter Lauf ${when}.`, 'ok'];
  }
  return [app.dryRun ? 'Bereit (Trockenlauf).' : 'Bereit.', 'ok'];
}
