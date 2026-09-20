// Einstieg: Fenster, Navigation, Tastatur, Start und Schließen.

import { api, appWindow } from './api.js';
import { openResultFolder, showFirstRunNotice } from './actions.js';
import { adoptRunning, finishedCount, refreshApp, settingsSaved } from './run.js';
import { addLog, locked, setView, state, subscribe } from './store.js';
import {
  $, $$, action, blocker, bullets, closeModal, el, errorBox, infoBox, modal, pointerAway, warnBox,
} from './ui.js';
import * as alerts from './views/alerts.js';
import * as portals from './views/portals.js';
import * as profile from './views/profile.js';
import * as runbar from './views/runbar.js';
import * as system from './views/system.js';

const VIEWS = {
  alerts: 'Job-Alerts',
  portals: 'Portal-Zugänge',
  profile: 'Beraterprofil',
  system: 'System',
};

/* ------------------------------------------------------------------ Tastatur */

// Gehaltene Enter- oder Leertaste löst nie mehrfach aus (auch nicht den Knopf des Folgedialogs).
document.addEventListener('keydown', (event) => {
  if (event.repeat && (event.key === 'Enter' || (event.key === ' ' && !event.target.matches('input')))) {
    event.preventDefault();
    event.stopPropagation();
  }
}, true);

/* ------------------------------------------------------------------ Fenster */

let maximizedTimer = 0;
/** Die Rückfrage „Beenden?“ ist offen. */
let asking = false;
/** Das Beenden läuft schon (das Backend räumt bis zu 10 s auf). */
let quitting = false;

async function syncMaximized() {
  const maximized = await appWindow.isMaximized().catch(() => false);
  const text = maximized ? 'Wiederherstellen' : 'Maximieren';
  $('#titlebar').classList.toggle('is-maximized', maximized);
  $('#win-max').title = text;
  $('#win-max').setAttribute('aria-label', text);
}

// Ein Aufruf je Größenänderung-Schub statt dutzender.
window.addEventListener('resize', () => {
  clearTimeout(maximizedTimer);
  maximizedTimer = setTimeout(syncMaximized, 100);
});
syncMaximized();

// Die Fensterknöpfe verschieben oder verstecken das Fenster selbst – danach zählt kein
// :hover mehr, bis sich der Zeiger wieder bewegt.
$('#win-min').addEventListener('click', () => {
  pointerAway();
  appWindow.minimize();
});
$('#win-max').addEventListener('click', () => {
  pointerAway();
  appWindow.toggleMaximize();
});
$('#win-close').addEventListener('click', () => {
  // Während der Rückfrage und während des Beendens bleibt ein zweiter Klick ohne Wirkung:
  // Das Backend schlösse sonst hart und bräche das Schreiben der Ergebnisse ab.
  if (!asking && !quitting) appWindow.close();
});

// Inaktives Fenster: Titelleiste blasser, wie bei Windows.
const syncActive = () => $('#titlebar').classList.toggle('is-inactive', !document.hasFocus());
window.addEventListener('focus', syncActive);
window.addEventListener('blur', syncActive);
syncActive();

// Nur während eines Laufs oder einer Anmeldung fragt das Backend nach; ein zweites
// Schließen binnen 10 s (z. B. Alt+F4) beendet auch ohne Antwort der Seite.
appWindow.onCloseRequested(async () => {
  if (asking || quitting) return;
  asking = true;
  // Die Rückfrage darf nicht hinter einem anderen Dialog warten.
  closeModal();
  const quit = await modal({
    id: 'quit',
    // Ist der Lauf (oder die Anmeldung) inzwischen fertig, entfällt die Frage.
    stale: () => !locked(),
    title: 'Beenden?',
    kind: 'warn',
    icon: 'question',
    body: state.busy
      ? 'Eine Aufgabe läuft noch.\n\nDer laufende Abruf wird beim Beenden abgebrochen; bereits Gespeichertes bleibt erhalten.'
      : 'Das freelance.de-Anmeldefenster ist noch offen.\n\nDie Anmeldung wird beim Beenden abgebrochen.',
    buttons: [
      { label: 'Weiterlaufen lassen', value: false, primary: true },
      { label: 'Beenden', value: true, kind: 'danger' },
    ],
  });
  asking = false;
  // Das Backend weiß jetzt: Die Seite antwortet. Ein weiteres ✕ fragt wieder nach.
  api.closeAnswered().catch(() => {});
  if (quit !== true) return;
  quitting = true;
  await settingsSaved();
  blocker('quitting', 'Wird beendet …', 'Der laufende Vorgang wird abgebrochen.');
  try {
    await api.quit();
  } catch (error) {
    quitting = false;
    closeModal('quitting');
    await errorBox('Beenden nicht möglich', error.message);
  }
});

/* ------------------------------------------------------------------ Navigation */

function showView(view) {
  for (const button of $$('#nav .nav-item')) {
    if (button.dataset.view === view) button.setAttribute('aria-current', 'page');
    else button.removeAttribute('aria-current');
  }
  for (const section of $$('.view')) section.classList.toggle('is-active', section.id === `view-${view}`);
  const title = VIEWS[view];
  $('#view-title').textContent = title;
  document.title = `Job-Alert-Monitor – ${title}`;
  setView(view);
}

/** Ansicht wechseln; ungespeicherte Gmail-Eingaben werden vorher geklärt. */
async function requestView(view) {
  if (view === state.view || !state.app) return;
  if (state.view === 'system' && !(await system.confirmLeave())) return;
  showView(view);
}

for (const button of $$('#nav .nav-item')) {
  button.addEventListener('click', () => requestView(button.dataset.view));
}
subscribe('navigate', requestView);
subscribe('openResults', openResultFolder);

/* ------------------------------------------------------------------ Start */

async function showResetReport(report) {
  if (!report.failed.length) {
    addLog('ok', 'Alles auf den Auslieferungszustand zurückgesetzt.');
    await infoBox('Alles zurückgesetzt', 'Die App ist jetzt im Auslieferungszustand.');
  } else {
    addLog('warn', 'Zurückgesetzt, aber Reste vorhanden.');
    await warnBox('Zurücksetzen nicht vollständig',
      `Diese Dateien ließen sich nicht löschen, meist weil ein Programm sie noch benutzt:\n\n${bullets(report.failed)}\n\n`
      + 'Das Programm schließen, das sie benutzt, und „Alles zurücksetzen“ noch einmal ausführen.');
  }
}

/** Startfehler: im Inhaltsbereich, mit „Erneut versuchen“. */
function showStartError(error) {
  const retry = el('button', { class: 'btn primary', type: 'button', text: 'Erneut versuchen' });
  action(retry, (busy) => busy(start()));
  $('.boot').replaceChildren(
    el('span', { class: 'state-title', text: 'Start nicht möglich' }),
    el('span', { text: error.message }),
    retry,
  );
  $('#run-status').textContent = 'Start nicht möglich.';
  retry.focus();
}

let started = false;
let booting = false;

async function start() {
  // Nie zweimal gleichzeitig: Zwei Startversuche könnten die Ansichten halb gebaut hinterlassen.
  if (booting) return;
  booting = true;
  try {
    await boot();
  } finally {
    booting = false;
  }
}

async function boot() {
  const before = finishedCount();
  let app;
  try {
    app = await refreshApp();
  } catch (error) {
    showStartError(error);
    return;
  }
  // Ansichten erst mit dem Zustand bauen – sie zeichnen sich sofort vollständig.
  if (!started) {
    started = true;
    runbar.build();
    alerts.build($('#view-alerts'));
    portals.build($('#view-portals'));
    profile.build($('#view-profile'));
    system.build($('#view-system'));
    $('#sidebar-foot').replaceChildren(app.dryRun ? el('span', { class: 'badge warn', text: 'Trockenlauf' }) : '');
  }
  showView('alerts');
  // Kam „Fertig“ schon über den Kanal, während der Zustand geladen wurde, ist der
  // Schnappschuss veraltet – dann nichts übernehmen.
  if (app.running && finishedCount() === before) adoptRunning(app.running);
  // Signal für die Selbstprüfung (--smoke): die Oberfläche steht.
  document.documentElement.dataset.ready = '1';
  if (app.resetReport) await showResetReport(app.resetReport);
  if (app.firstRunNotice) await showFirstRunNotice();
}

start();
