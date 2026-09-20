// Der Lauf: starten, zuhören, abschließen. Gemeldet wird über genau einen Kanal –
// die Statusleiste. Ein Dialog erscheint nur, wenn der Nutzer etwas selbst angestoßen hat
// und es schiefging.

import { api } from './api.js';
import {
  addLog, emit, locked, resetRun, setApp, setBusy, setCancelling, setSelection, state, updateRun,
} from './store.js';
import { errorDialog, formatTime, plural, pruneDialog } from './ui.js';

/** Zustand neu laden. Antworten eines überholten Aufrufs werden verworfen. */
let seq = 0;

export async function refreshApp() {
  const mine = ++seq;
  const app = await api.appState(onEvent);
  if (mine === seq || !state.app) setApp(app);
  pruneDialog();
  return app;
}

export async function saveSettings(patch) {
  const before = state.app.settings;
  const input = { ...before, ...patch };
  setApp({ ...state.app, settings: input });
  try {
    const saved = await api.saveSettings(input);
    setApp({ ...state.app, settings: saved });
  } catch (error) {
    setApp({ ...state.app, settings: before });
    await errorDialog('Einstellung nicht gespeichert', error.message);
  }
}

/* ------------------------------------------------------------------ Starten */

const REQUESTS = {
  // Postfach abrufen: Mails lesen, Jobdetails holen, Dateien schreiben.
  scan: { scan: true, fetch: true, export: true, scope: 'new' },
  // Ganzes Postfach durchsuchen (Wartung).
  all: { scan: true, fetch: true, export: true, scope: 'all' },
  // Nur die fehlenden Jobdetails nachholen.
  fetch: { scan: false, fetch: true, export: true, scope: 'new' },
};

export async function startRun(kind, jobs = null) {
  if (locked()) return;
  const base = REQUESTS[kind] ?? REQUESTS.scan;
  const request = { ...base, portals: state.app.settings.portals, jobs };
  resetRun(kind === 'all' ? 'scan' : kind);
  updateRun({ status: 'Wird gestartet …' });
  setBusy(true);
  try {
    await api.startRun(request, onEvent);
  } catch (error) {
    setBusy(false);
    await errorDialog('Abruf nicht gestartet', error.message);
  }
}

export async function cancelRun() {
  if (!state.busy || state.cancelling) return;
  setCancelling(true);
  try {
    await api.cancelRun();
  } catch (error) {
    setCancelling(false);
    addLog('warn', `Abbrechen nicht möglich: ${error.message}`);
  }
}

/** Beim Start: Läuft schon ein Lauf, hängt sich die Seite wieder an ihn. */
export function adoptRunning(running) {
  resetRun(running.progress?.step === 'fetch' ? 'fetch' : 'scan');
  for (const event of running.log) {
    if (event.type === 'log') addLog(event.level, event.text, event.at);
    else if (event.type === 'portalStopped') state.run.stops[event.portal] = event;
  }
  updateRun({
    status: running.status,
    until: running.statusUntil ?? null,
    done: running.progress?.done ?? 0,
    total: running.progress?.total ?? 0,
  });
  setBusy(true);
}

/* ------------------------------------------------------------------ Ereignisse */

const portalLabel = (key) => state.app?.portals.find((p) => p.portal === key)?.label ?? key;

function onEvent(event) {
  switch (event.type) {
    case 'log':
      addLog(event.level, event.text, event.at);
      break;
    case 'status':
      updateRun({ status: event.text, until: event.until ?? null });
      break;
    case 'progress':
      updateRun({ done: event.done, total: event.total });
      break;
    case 'alert':
      onAlert(event);
      break;
    case 'jobUpdated':
      emit('job', event.job);
      break;
    case 'portalStopped':
      state.run.stops[event.portal] = event;
      emit('run', state.run);
      break;
    case 'loginNeeded':
      state.run.loginWaiting[event.portal] = event.waiting;
      updateRun({ status: event.waiting ? `Anmeldefenster offen – ${portalLabel(event.portal)}` : state.run.status });
      break;
    case 'finished':
      finished(event);
      break;
    default:
      break;
  }
}

function onAlert(event) {
  const head = `${portalLabel(event.portal)} · ${event.date ? formatTime(event.date) : 'ohne Datum'} · „${event.subject}“`;
  if (event.postings === 0) {
    state.run.emptyAlerts.push(event);
    addLog('warn', `${head} · keine Jobs erkannt`);
  } else {
    addLog('info', `${head} · ${plural(event.postings, 'Job', 'Jobs')}`);
  }
  emit('run', state.run);
  emit('jobs', 'alert');
}

/* ------------------------------------------------------------------ Abschluss */

async function finished(event) {
  const summary = event.summary;
  updateRun({ status: 'Ergebnis wird übernommen …', summary });
  const fresh = await refreshApp().then(() => state.app.lastRun).catch(() => null);
  // Erst der Abschlusssatz, dann den Kreisel weg: Andersherum stünde für einen Bildschritt
  // der ruhende Punkt neben „Ergebnis wird übernommen …“.
  updateRun({ summary, status: summaryLine(summary), until: null, level: levelOf(summary) });
  setBusy(false);
  emit('jobs', 'finished');

  // Fehler des Laufs stoppen nicht – sie stehen in der Statusleiste und im Verlauf.
  // Nur ein Export-Fehler betrifft eine Datei, die der Nutzer gleich öffnen will.
  if (summary?.export?.error) await errorDialog('Ergebnisdateien nicht geschrieben', summary.export.error);
  void fresh;
}

function levelOf(summary) {
  if (!summary) return '';
  if (summary.outcome?.kind === 'failed') return 'error';
  if (summary.outcome?.kind === 'cancelled') return 'warn';
  const stops = Object.values(summary.fetch?.perPortal ?? {}).some((c) => c.stop);
  const failed = Object.values(summary.fetch?.perPortal ?? {}).some((c) => c.failed);
  return stops || failed ? 'warn' : 'ok';
}

/** Ein Satz, der den Lauf zusammenfasst – mehr braucht die Statusleiste nicht. */
export function summaryLine(summary) {
  if (!summary) return 'Bereit';
  if (summary.outcome?.kind === 'failed') return summary.outcome.message ?? 'Abruf fehlgeschlagen';
  const scan = summary.scan;
  const ok = Object.values(summary.fetch?.perPortal ?? {}).reduce((sum, c) => sum + c.ok, 0);
  const parts = [];
  if (scan) {
    if (scan.alertMails === 0) parts.push('keine Alert-Mails');
    else parts.push(plural(scan.new, 'neuer Job', 'neue Jobs'));
  }
  if (ok) parts.push(plural(ok, 'Jobdetail', 'Jobdetails'));
  if (!parts.length) parts.push('nichts Neues');
  const when = summary.finishedAt ? formatTime(summary.finishedAt) : '';
  const head = summary.outcome?.kind === 'cancelled' ? 'Abgebrochen' : when;
  return [head, parts.join(' · ')].filter(Boolean).join(' · ');
}

/** Nach einem Lauf öffnet die App den ersten neuen Job – das ist der nächste Schritt. */
export function firstFresh() {
  return state.run.fresh[0] ?? null;
}

export { setSelection };
