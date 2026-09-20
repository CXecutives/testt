// Start und Gerüst: Titelleiste, Werkzeugleiste, zwei Seiten, Statusleiste.

import { api, appWindow } from './api.js';
import { install as installInput } from './input.js';
import { adoptRunning, refreshApp, startRun } from './run.js';
import { locked, setFilter, setSearch, setView, state, subscribe } from './store.js';
import {
  $, blocker, brandMark, button, captionButton, el, field, iconButton, pointerAway, segmented,
} from './ui.js';
import * as jobs from './views/jobs.js';
import * as settings from './views/settings.js';
import * as status from './views/status.js';
import { steps as stepsList } from './ui.js';

const n = {};
const isWindows = !navigator.userAgent.includes('Macintosh');

/* ------------------------------------------------------------------ Titelleiste */

function buildTitlebar() {
  const bar = $('#titlebar');
  bar.append(
    el('span', { class: 'brand', 'data-tauri-drag-region': true }, brandMark(),
      el('span', { class: 'brand-text', text: 'Job-Alert-Monitor', 'data-tauri-drag-region': true })),
    el('span', { class: 'titlebar-fill', 'data-tauri-drag-region': true }),
    el('span', { class: 'mode-tag', id: 'mode-tag', text: 'Trockenlauf', hidden: true }));
  if (!isWindows) return;

  // Eigene Fensterknöpfe nur unter Windows; macOS zeichnet seine Ampel selbst.
  n.max = captionButton({ id: 'win-max', label: 'Maximieren', paths: ['M1.5 1.5h7v7h-7z'] });
  bar.append(el('div', { class: 'win-buttons' },
    captionButton({ id: 'win-min', label: 'Minimieren', paths: ['M1 5h8'] }),
    n.max,
    captionButton({ id: 'win-close', label: 'Schließen', paths: ['M1.5 1.5l7 7', 'M8.5 1.5l-7 7'], close: true })));

  $('#win-min').addEventListener('click', () => { pointerAway(); appWindow.minimize(); });
  n.max.addEventListener('click', () => { pointerAway(); appWindow.toggleMaximize(); });
  $('#win-close').addEventListener('click', () => appWindow.close());

  // Ein Größenwechsel kommt als Schwall; einmal nachfragen genügt.
  let pending = 0;
  const sync = () => {
    clearTimeout(pending);
    pending = setTimeout(async () => {
      const on = await appWindow.isMaximized().catch(() => false);
      n.max.title = on ? 'Wiederherstellen' : 'Maximieren';
    }, 80);
  };
  addEventListener('resize', sync);
  sync();
}

/* ------------------------------------------------------------------ Werkzeugleiste */

function buildToolbar() {
  n.run = button({
    id: 'run', label: 'Abrufen', variant: 'primary', icon: 'refresh',
    onAction: () => startRun('scan'),
  });
  n.filter = segmented({
    options: [{ value: 'new', label: 'Neu', count: 0 }, { value: 'all', label: 'Alle', count: 0 }],
    value: 'new',
    onChange: setFilter,
  });
  n.search = field({ id: 'search', placeholder: 'Suchen', compact: true, onInput: debounce(setSearch, 250) });
  n.search.node.classList.add('search');
  n.folder = iconButton({
    icon: 'folder', label: 'Ergebnisordner öffnen',
    onAction: () => api.openTarget({ kind: 'resultFolder' }).catch(() => {}),
  });
  n.gear = iconButton({
    icon: 'gear', label: 'Einstellungen',
    onClick: () => go(state.view === 'settings' ? 'jobs' : 'settings'),
  });
  $('#toolbar').append(n.run, n.filter.node, el('span', { class: 'toolbar-fill' }),
    n.search.node, n.folder, n.gear);
}

function debounce(fn, ms) {
  let timer = 0;
  return (value) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(value), ms);
  };
}

// Seitenwechsel blendet über: `display: none` → `flex` startet die Animation der Seite
// von selbst. Die View-Transitions-API wäre hübscher, fror aber unter WebView2 das Bild
// ein, wenn während des Übergangs gezeichnet wurde – ein Effekt ist das nicht wert.
const go = (view) => setView(view);

function renderShell() {
  const app = state.app;
  $('#page-jobs').hidden = state.view !== 'jobs';
  $('#page-settings').hidden = state.view !== 'settings';
  n.gear.title = state.view === 'settings' ? 'Zurück zu den Jobs' : 'Einstellungen';
  const why = locked() ? 'Erst nach dem laufenden Vorgang möglich' : null;
  const ready = Boolean(app?.gmailUser) && (app?.settings.portals.length ?? 0) > 0;
  n.run.disabled = Boolean(why) || !ready;
  n.run.title = why ?? (app && !app.gmailUser ? 'Erst das Postfach verbinden'
    : app && !app.settings.portals.length ? 'Kein Portal gewählt' : '');
  // „Neu“ zählt, was der Filter auch zeigt: die Jobs des letzten Laufs. Nach einem Lauf
  // steht das in state.run.fresh, davor in der gespeicherten Zusammenfassung.
  const fresh = state.run.summary ? state.run.fresh.length : (app?.lastRun?.scan?.new ?? 0);
  n.filter.setCounts({ new: fresh, all: app?.jobsTotal ?? 0 });
  $('#mode-tag').hidden = !app?.dryRun;
}

/* ------------------------------------------------------------------ Erststart */

function renderSetup() {
  const app = state.app;
  if (!app || state.view !== 'jobs') return;
  const done = Boolean(app.gmailUser);
  if (done || app.jobsTotal) { n.setup?.remove(); n.setup = null; return; }
  if (n.setup) return;
  const list = stepsList([
    { title: 'Postfach verbinden', done: false, action: { label: 'Einrichten', primary: true, onClick: () => go('settings') } },
    { title: 'Portale anmelden – optional', done: false, action: { label: 'Öffnen', onClick: () => go('settings') } },
    { title: 'Ersten Abruf starten', done: false },
  ]);
  n.setup = el('div', { class: 'reader-inner' }, list.node);
  $('#reader').replaceChildren(n.setup);
}

/* ------------------------------------------------------------------ Start */

async function start() {
  const app = await refreshApp();
  document.documentElement.dataset.platform = app.platform;
  state.gmailForm.user = app.gmailUser ?? '';
  if (app.running) adoptRunning(app.running);
  await jobs.load();
  renderShell();
  renderSetup();
  $('#boot').hidden = true;
  $('#app').hidden = false;
}

function bootError(error) {
  const retry = button({ label: 'Erneut versuchen', variant: 'primary', onAction: () => boot() });
  $('#boot').replaceChildren(el('span', { text: `Start nicht möglich: ${error.message}` }), retry);
  $('#boot').hidden = false;
}

async function boot() {
  $('#boot').replaceChildren(el('span', { class: 'spinner' }));
  try {
    await start();
  } catch (error) {
    bootError(error);
  }
}

installInput();
buildTitlebar();
buildToolbar();
jobs.build($('#page-jobs'));
jobs.onSettings(() => go('settings'));
settings.build($('#page-settings'));
status.build($('#statusbar'), $('#panel'));

subscribe('app', renderShell);
subscribe('busy', renderShell);
subscribe('view', () => { renderShell(); renderSetup(); });
subscribe('run', renderShell);

appWindow.onClosing(() => blocker('quit', 'Wird beendet …', 'Der laufende Abruf wird sauber abgebrochen.'));

void boot();
