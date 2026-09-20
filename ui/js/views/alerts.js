// Ansicht „Job-Alerts“: Kopfaktionen, Ergebnisse (jobs.js), Einstellungen, Verlauf.

import { saveSettings, startRun } from '../run.js';
import { locked, lockedWhy, state, subscribe } from '../store.js';
import { $, action, el, errorBox, formatClock, markBusy, plural, toast } from '../ui.js';
import * as jobs from './jobs.js';

const SCOPES = [['new', 'Neu seit letztem Lauf'], ['week', 'Letzte 7 Tage'], ['all', 'Alle']];
const FORMATS = [['xlsx', 'Excel (.xlsx)'], ['csv', 'CSV'], ['none', 'Keine Datei']];

const label = (pairs, value) => pairs.find(([v]) => v === value)?.[1] ?? value;
const n = {};
let settingsOpen = false;
/** Folgt der Verlauf den neuen Zeilen? (Wer nach oben scrollt, bleibt oben.) */
let follow = true;

export function build(root) {
  root.classList.add('split');
  n.side = el('div', { class: 'col-side' });
  const { results, detail } = jobs.build(n.side);
  n.side.append(settingsCard(), logCard(), detail);
  root.append(el('div', { class: 'col-main' }, results), n.side);

  n.start = el('button', { class: 'btn primary', type: 'button', text: 'Postfach abrufen', onclick: onStart });
  n.desc = el('button', { class: 'btn', type: 'button', text: 'Jobdetails extrahieren', onclick: () => startRun('fetch') });
  n.actions = el('div', { class: 'topbar-actions' }, n.desc, n.start);
  $('#topbar-actions').append(n.actions);

  subscribe('view', (view) => {
    n.actions.hidden = view !== 'alerts';
  });
  subscribe('app', () => {
    renderSettings();
    renderActions();
  });
  subscribe('busy', () => {
    renderSettings();
    renderActions();
  });
  subscribe('run', renderActions);
  subscribe('log', appendLog);
  for (const entry of state.log) appendLog({ entry, dropped: 0 });
  renderSettings();
  renderActions();
}

/* ------------------------------------------------------------------ Kopfaktionen */

async function onStart() {
  if (!state.app.dryRun && !state.app.gmailUser) {
    await errorBox('Abruf nicht gestartet', 'Es fehlen die Gmail-Zugangsdaten. Unter „System“ Adresse und App-Passwort eintragen und speichern.');
    return;
  }
  jobs.showNew();
  await startRun('scan');
}

function renderActions() {
  const app = state.app;
  const chosen = app.portals.filter((p) => app.settings.portals.includes(p.portal));
  const open = chosen.reduce((sum, p) => sum + p.open + p.failed, 0);
  const due = chosen.reduce((sum, p) => sum + p.due, 0);
  const none = app.settings.portals.length === 0;
  const kind = state.busy ? state.run.kind : null;
  const busy = lockedWhy();
  n.start.disabled = Boolean(busy) || none;
  markBusy(n.start, kind === 'scan');
  n.start.title = busy ?? (none ? 'Bitte mindestens ein Portal wählen' : '');
  n.desc.disabled = Boolean(busy) || due === 0;
  markBusy(n.desc, kind === 'fetch' || kind === 'job');
  n.desc.title = busy ?? (none ? 'Bitte mindestens ein Portal wählen'
    : !app.jobsTotal ? 'Erst das Postfach abrufen'
      : !open ? 'Alle Jobdetails liegen vor'
        : !due ? 'Keine Jobdetails abrufbar: Portal pausiert oder Obergrenze erreicht, '
          + 'letzter Versuch jünger als 12 Stunden oder Mail älter als 30 Tage'
          : `${plural(due, 'Jobdetail', 'Jobdetails')} ${due === 1 ? 'kann' : 'können'} jetzt geholt werden`);
}

/* ------------------------------------------------------------------ Einstellungen */

function settingsCard() {
  n.summary = el('p');
  n.toggle = el('button', {
    class: 'btn', type: 'button', 'aria-expanded': 'false', 'aria-controls': 'settings-body',
    onclick: () => {
      settingsOpen = !settingsOpen;
      renderSettings();
    },
  });
  const choice = (type, name, value, text, onchange) => el('label', { class: 'choice' },
    el('input', { type, name, value, onchange }), el('span', { text }));
  const group = (title, choices, hint) => el('div', { class: 'field', role: 'group', 'aria-label': title },
    el('span', { class: 'field-label', text: title }),
    el('div', { class: 'choices' }, choices),
    el('p', { class: 'hint', text: hint }));

  n.scope = SCOPES.map(([value, text]) => choice('radio', 'scope', value, text, () => saveSettings({ scope: value })));
  n.portals = state.app.portals.map((p) => choice('checkbox', 'portal', p.portal, p.label, (event) => {
    // Klickfolge merken: angehakt ⇒ hinten anfügen, abgehakt ⇒ entfernen.
    const rest = state.app.settings.portals.filter((id) => id !== p.portal);
    saveSettings({ portals: event.target.checked ? [...rest, p.portal] : rest });
  }));
  n.format = FORMATS.map(([value, text]) => choice('radio', 'format', value, text, () => saveSettings({ format: value })));
  n.inputs = [...n.scope, ...n.portals, ...n.format].map((node) => node.firstChild);

  n.collapse = el('div', { class: 'collapse is-closed', id: 'settings-body' },
    el('div', null,
      el('div', { class: 'settings-body' },
        group('Mails prüfen', n.scope, '„Neu seit letztem Lauf“ liest ab dem letzten erfolgreichen Abruf (einen Tag überlappend, beim ersten Mal 7 Tage).'),
        group('Portale', n.portals, 'Ohne Auswahl kein Abruf.'),
        group('Übersicht speichern als', n.format, 'Jobs und Textdateien werden immer gespeichert.'))));

  return el('div', { class: 'card settings' },
    el('div', { class: 'card-head' },
      el('div', { class: 'card-title' }, el('h2', { text: 'Umfang und Portale' }), n.summary),
      n.toggle),
    n.collapse);
}

function renderSettings() {
  const { scope, portals, format } = state.app.settings;
  n.collapse.classList.toggle('is-closed', !settingsOpen);
  n.toggle.textContent = settingsOpen ? 'Fertig' : 'Ändern';
  n.toggle.setAttribute('aria-expanded', String(settingsOpen));
  const sources = state.app.portals.filter((p) => portals.includes(p.portal)).map((p) => p.label).join(', ') || 'kein Portal';
  n.summary.textContent = `${label(SCOPES, scope)} · ${sources} · ${label(FORMATS, format)}`;
  n.summary.title = n.summary.textContent;
  for (const input of n.inputs) {
    const value = input.value;
    input.checked = input.name === 'scope' ? value === scope : input.name === 'format' ? value === format : portals.includes(value);
    input.disabled = locked();
  }
}

/* ------------------------------------------------------------------ Verlauf */

function logCard() {
  n.log = el('div', {
    class: 'log', id: 'log', role: 'log', 'aria-label': 'Verlauf',
    // Mitlaufen merken: Bei verborgener Karte sind alle Maße 0 – aus der Geometrie gelesen,
    // bliebe der Verlauf danach dauerhaft oben stehen.
    onscroll: () => {
      follow = n.log.scrollHeight - n.log.scrollTop - n.log.clientHeight < 24;
    },
  });
  n.logEmpty = el('div', { class: 'state' }, el('span', { class: 'state-title', text: 'Noch keine Meldungen' }), 'Hier steht, was die App gerade tut.');
  n.log.append(n.logEmpty);
  // Wird die Karte wieder sichtbar (Ansichtswechsel, Detail geschlossen), springt der Verlauf nach.
  new ResizeObserver(() => {
    if (follow) n.log.scrollTop = n.log.scrollHeight;
  }).observe(n.log);
  n.copy = el('button', { class: 'btn', type: 'button', text: 'Verlauf kopieren' });
  action(n.copy, copyLog);
  renderCopy();
  return el('div', { class: 'card log-card' },
    el('div', { class: 'card-head' }, el('div', { class: 'card-title' }, el('h2', { text: 'Verlauf' }))),
    el('div', { class: 'card-body flush' }, n.log),
    el('div', { class: 'card-foot' }, n.copy));
}

function appendLog({ entry, dropped }) {
  n.logEmpty.remove();
  for (let i = 0; i < dropped && n.log.firstChild; i += 1) n.log.firstChild.remove();
  n.log.append(el('div', { class: `log-line ${entry.level}` },
    el('time', { text: formatClock(entry.at) }), el('span', { text: entry.text })));
  if (follow) n.log.scrollTop = n.log.scrollHeight;
  renderCopy();
}

function renderCopy() {
  n.copy.disabled = !state.log.length;
  n.copy.title = state.log.length ? '' : 'Noch keine Meldungen';
}

async function copyLog(busy) {
  const text = state.log.map((e) => `${formatClock(e.at)}  ${e.text}`).join('\n');
  try {
    await busy(navigator.clipboard.writeText(text));
  } catch {
    await errorBox('Kopieren nicht möglich', 'Die Zwischenablage ist gerade nicht erreichbar.');
    return;
  }
  toast('Verlauf kopiert.', 'ok');
}
