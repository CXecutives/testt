// Laufsteuerung: Zustand laden, Einstellungen speichern, Lauf starten und abbrechen,
// Ereignisse des Laufs, Abschlussmeldung.

import { api } from './api.js';
import {
  addLog, emit, locked, resetRun, setApp, setBusy, setCancelling, state, updateRun,
} from './store.js';
import { closeModal, errorBox, formatTime, infoBox, modal, plural, toast } from './ui.js';

export const portalLabel = (portal) => state.app?.portals.find((p) => p.portal === portal)?.label ?? portal;

/* ------------------------------------------------------------------ Zustand */

let refreshSeq = 0;
let settingsVersion = 0;

/**
 * Zustand neu laden; hängt die Seite an einen laufenden Lauf an (je Aufruf ein neuer Kanal).
 * Nur die Antwort des neuesten Aufrufs gilt, und Einstellungen, die inzwischen geändert
 * wurden, überschreibt eine ältere Antwort nicht.
 */
export async function refreshApp() {
  const seq = ++refreshSeq;
  const version = settingsVersion;
  const app = await api.appState(onEvent);
  // Überholt: Die neuere Antwort gilt – außer beim Start, wo es noch gar keinen Zustand gibt.
  if (seq !== refreshSeq && state.app) return state.app;
  if (version !== settingsVersion && state.app) app.settings = state.app.settings;
  setApp(app);
  return app;
}

// Einstellungen werden der Reihe nach gespeichert; Start und Beenden warten darauf.
let pending = Promise.resolve();

export function saveSettings(patch) {
  settingsVersion += 1;
  const current = state.app.settings;
  const input = { format: current.format, scope: current.scope, portals: current.portals, firstRunSeen: false, ...patch };
  // Sofort anzeigen, dann speichern – die Antwort des Backends ist maßgeblich.
  state.app.settings = { ...current, ...input, firstRunSeen: current.firstRunSeen || input.firstRunSeen };
  emit('app', state.app);
  pending = pending
    .then(() => api.saveSettings(input))
    .then((saved) => {
      state.app.settings = saved;
      emit('app', state.app);
    })
    .catch((error) => {
      // Zurück auf den Stand des Backends; die Meldung wartet nicht (Start und Beenden warten auf `pending`).
      settingsVersion += 1;
      refreshApp().catch(() => {});
      errorBox('Einstellungen nicht gespeichert', error.message);
    });
  return pending;
}

export const settingsSaved = () => pending;

/* ------------------------------------------------------------------ Lauf */

let starting = false;

/**
 * kind: 'scan' (ein Klick: Postfach → Jobdetails → Export) · 'fetch' (offene Jobdetails)
 * · 'job' (Details eines Jobs, `jobs` = [key]).
 */
export async function startRun(kind, jobs = null) {
  // Ein zweiter Klick, während die Einstellungen noch gespeichert werden, startet nichts.
  if (locked() || starting) return;
  starting = true;
  try {
    await settingsSaved();
    const { scope, portals } = state.app.settings;
    const request = { scan: kind === 'scan', fetch: true, export: true, scope, portals, jobs };
    const previous = state.run;
    resetRun(kind);
    updateRun({ status: kind === 'scan' ? 'Postfach wird abgerufen…' : 'Jobdetails werden vorbereitet…' });
    if (state.app.dryRun && kind === 'scan') addLog('warn', 'Trockenlauf – es wird kein Postfach geöffnet.');
    setBusy(true);
    emit('jobs', 'start');
    try {
      await api.startRun(request, onEvent);
    } catch (error) {
      // Nicht gestartet: der vorige Laufstand bleibt sichtbar.
      state.run = previous;
      setBusy(false);
      emit('run', state.run);
      await errorBox(kind === 'scan' ? 'Abruf nicht gestartet' : 'Extraktion nicht gestartet', error.message);
    }
  } finally {
    starting = false;
  }
}

/** Bricht den Lauf oder die manuelle Anmeldung ab. */
export async function cancelRun() {
  if (!locked() || state.cancelling) return;
  setCancelling(true);
  if (state.busy) addLog('warn', 'Abbruch angefordert – der Lauf endet nach dem aktuellen Schritt.');
  try {
    await api.cancelRun();
  } catch (error) {
    setCancelling(false);
    toast(`Abbrechen nicht möglich: ${error.message}`, 'error');
  }
}

/** Beim Start: einen laufenden Lauf übernehmen (Verlauf nur beim ersten Laden abspielen). */
export function adoptRunning(running) {
  resetRun(running.progress?.step === 'fetch' ? 'fetch' : 'scan');
  for (const event of running.log) {
    if (event.type === 'log') addLog(event.level, event.text, event.at);
    else if (event.type === 'portalStopped') state.run.stops[event.portal] = event;
  }
  updateRun({ status: running.status, until: running.statusUntil ?? null, progress: running.progress });
  setBusy(true);
}

/* ------------------------------------------------------------------ Ereignisse */

function onEvent(event) {
  switch (event.type) {
    case 'log':
      addLog(event.level, event.text, event.at);
      break;
    case 'status':
      updateRun({ status: event.text, until: event.until ?? null });
      break;
    case 'progress':
      updateRun({ progress: event });
      break;
    case 'alert':
      onAlert(event);
      break;
    case 'jobUpdated':
      if (['failed', 'unfetchable', 'gone'].includes(event.job.status)) {
        addLog('warn', `Keine Jobdetails für ${event.job.title}: ${event.job.descError || event.job.statusText}`);
      }
      emit('job', event.job);
      break;
    case 'portalStopped':
      state.run.stops[event.portal] = event;
      emit('run', state.run);
      break;
    case 'loginNeeded':
      state.run.loginWaiting[event.portal] = event.waiting;
      emit('run', state.run);
      loginDialog(event.waiting);
      break;
    case 'finished':
      finished(event);
      break;
    default:
      break;
  }
}

function onAlert(event) {
  const counts = state.run.alertCounts[event.portal] ?? [0, 0];
  state.run.alertCounts[event.portal] = [counts[0] + 1, counts[1] + event.postings];
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

let loginShown = false;

function loginDialog(waiting) {
  if (!waiting) {
    if (loginShown) closeModal('login');
    loginShown = false;
    return;
  }
  if (loginShown) return;
  loginShown = true;
  modal({
    id: 'login',
    title: 'Anmeldung nötig',
    kind: 'warn',
    body: 'freelance.de zeigt Projekttexte nur angemeldeten Nutzern.\n\n'
      + 'Bitte jetzt im geöffneten Fenster selbst anmelden („angemeldet bleiben“ aktiviert lassen). '
      + 'Die App trägt keine Zugangsdaten ein und speichert kein freelance.de-Passwort.\n\n'
      + 'Sobald die Anmeldung erkannt ist, läuft die Extraktion von selbst weiter – die App wartet höchstens 5 Minuten. '
      + '„Abbrechen“ beendet das Warten.',
    buttons: [
      { label: 'OK', value: true, primary: true },
      { label: 'Abbrechen', value: 'cancel', kind: 'danger' },
    ],
  }).then((value) => {
    loginShown = false;
    if (value === 'cancel') cancelRun();
  });
}

/* ------------------------------------------------------------------ Abschluss */

let finishedRuns = 0;

/** Wie viele Läufe diese Seite enden sah (der Start übernimmt keinen schon beendeten Lauf). */
export const finishedCount = () => finishedRuns;

async function finished(summary) {
  finishedRuns += 1;
  const kind = state.run.kind;
  closeModal('quit');
  loginDialog(false);
  const outcome = summary.outcome.kind;
  updateRun({
    summary,
    status: 'Ergebnis wird übernommen…',
    until: null,
    summaryLine: summaryLine(kind, summary),
    summaryLevel: outcome === 'failed' ? 'error' : outcome === 'cancelled' ? 'warn' : 'ok',
  });
  try {
    await refreshApp();
  } catch (error) {
    addLog('error', `Zustand nicht geladen: ${error.message}`);
  }
  // Erst mit dem neuen Zustand entsperren – sonst startet ein schneller Klick mit dem alten.
  setBusy(false);
  emit('jobs', 'finished');
  await finalDialog(kind, summary);
}

/** „LinkedIn 5 (2 Mails), freelancermap.de 3 (1 Mail)“ – oder „keine Alerts“. */
function alertSummary() {
  const parts = Object.entries(state.run.alertCounts)
    .filter(([, [mails]]) => mails > 0)
    .map(([portal, [mails, entries]]) => `${portalLabel(portal)} ${entries} (${mails} ${mails === 1 ? 'Mail' : 'Mails'})`);
  return parts.length ? parts.join(', ') : 'keine Alert-Mails';
}

function fetchTotals(summary) {
  const counts = Object.values(summary.fetch?.perPortal ?? {});
  const sum = (pick) => counts.reduce((total, c) => total + pick(c), 0);
  return { ok: sum((c) => c.ok), failed: sum((c) => c.failed), gone: sum((c) => c.gone), skipped: sum((c) => c.skipped) };
}

/** Abschlusszeile für die Laufleiste. */
function summaryLine(kind, summary) {
  const outcome = summary.outcome.kind;
  if (outcome === 'failed') return 'Fehler – siehe Verlauf.';
  const note = outcome === 'cancelled' ? ' (abgebrochen, Teilergebnis gespeichert)' : '';
  const totals = fetchTotals(summary);
  const scan = summary.scan;
  if (kind === 'scan' && scan) {
    if (scan.alertMails === 0) return `Fertig – keine Alert-Mails${note}.`;
    const skipped = [];
    if (scan.knownBefore) skipped.push(`${plural(scan.knownBefore, 'Job', 'Jobs')} bereits bekannt`);
    if (scan.dupInRun) skipped.push(`${scan.dupInRun} doppelt`);
    const skippedNote = skipped.length ? ` (${skipped.join(', ')})` : '';
    const fetched = totals.ok ? `, ${plural(totals.ok, 'Jobdetail', 'Jobdetails')} geholt` : '';
    const alerts = Object.keys(state.run.alertCounts).length ? alertSummary() : plural(scan.alertMails, 'Alert-Mail', 'Alert-Mails');
    return `Fertig – ${alerts}: ${scan.new} neu${skippedNote}${fetched}${note}.`;
  }
  let line = `Fertig – ${plural(totals.ok, 'Jobdetail', 'Jobdetails')} gespeichert`;
  if (totals.failed) line += `, ${totals.failed} fehlgeschlagen`;
  if (totals.skipped) line += `, ${totals.skipped} übersprungen`;
  return `${line}${note}.`;
}

/** Abrufblock: Jobdetails, Stopps, Textdateien – Teil jeder Abschlussmeldung. */
function fetchLines(summary) {
  const lines = [];
  const fetch = summary.fetch;
  const stops = Object.values(fetch?.perPortal ?? {}).map((c) => c.stop).filter(Boolean);
  if (fetch && (fetch.queued > 0 || stops.length)) {
    const totals = fetchTotals(summary);
    lines.push(totals.ok ? `${plural(totals.ok, 'Jobdetail', 'Jobdetails')} gespeichert.` : 'Keine Jobdetails gespeichert.');
    for (const text of stops) lines.push(`• ${text}`);
    const failed = totals.failed + totals.gone;
    if (failed) lines.push(`${plural(failed, 'Seite', 'Seiten')} ohne Jobdetails.`);
    // Gezählt wird für die gewählten Portale – nur deren Jobs holt „Jobdetails extrahieren“.
    const chosen = (state.app?.portals ?? []).filter((p) => state.app.settings.portals.includes(p.portal));
    const open = chosen.reduce((sum, p) => sum + p.open + p.failed, 0);
    const given = chosen.reduce((sum, p) => sum + p.unfetchable, 0);
    const due = chosen.reduce((sum, p) => sum + p.due, 0);
    if (open) {
      lines.push(`${plural(open, 'Job', 'Jobs')} ohne Jobdetails ${open === 1 ? 'geht' : 'gehen'} nicht ins Matching.`);
      lines.push(due
        ? '„Jobdetails extrahieren“ versucht nur die fehlenden; pausierte Portale erst nach Ablauf der Pause.'
        : 'Ein neuer Versuch ist nach Ablauf von Pause oder Obergrenze möglich, nach einem Fehlschlag frühestens 12 Stunden später. Jobs aus Mails älter als 30 Tage holt „Details holen“ einzeln.');
    }
    if (given) lines.push(`${plural(given, 'Job', 'Jobs')} nach mehreren Fehlversuchen aufgegeben – „Details holen“ versucht ${given === 1 ? 'ihn' : 'sie'} einzeln.`);
  }
  const exp = summary.export;
  if (exp?.txtWritten) lines.push(`${plural(exp.txtWritten, 'neue Textdatei', 'neue Textdateien')} geschrieben.`);
  if (exp?.txtFailedCount) lines.push(`${plural(exp.txtFailedCount, 'Textdatei ließ', 'Textdateien ließen')} sich nicht schreiben (der nächste Lauf versucht es erneut).`);
  if (exp?.backup) lines.push(`Fremde Übersichtsdatei gesichert: ${exp.backup}`);
  if (summary.scan?.mailsDefective) lines.push(`${plural(summary.scan.mailsDefective, 'Mail war', 'Mails waren')} nicht lesbar und wurde${summary.scan.mailsDefective === 1 ? '' : 'n'} übersprungen.`);
  if (summary.dryRun) lines.push('Trockenlauf – es wurde nichts gespeichert.');
  return lines;
}

async function finalDialog(kind, summary) {
  const { outcome, scan } = summary;
  const cancelled = outcome.kind === 'cancelled';
  const lines = fetchLines(summary);
  const text = (...head) => [...head, ...(lines.length ? ['', ...lines] : [])].join('\n');
  const wrote = Boolean(summary.export?.overview || summary.export?.txtWritten);
  const withFolder = wrote
    ? [{ label: 'OK', value: true, primary: true }, { label: 'Ergebnisordner öffnen', value: 'folder' }]
    : undefined;
  let choice = null;

  if (outcome.kind === 'failed') {
    const toSystem = ['mailAuth', 'mailMissing'].includes(outcome.error);
    choice = await modal({
      title: kind === 'scan' ? 'Postfach-Abruf nicht abgeschlossen' : 'Jobdetails nicht abgeschlossen',
      kind: 'error',
      body: outcome.message,
      buttons: toSystem
        ? [{ label: 'Zu „System“', value: 'system', primary: true }, { label: 'OK', value: true }]
        : [{ label: 'OK', value: true, primary: true }],
    });
    if (choice === 'system') emit('navigate', 'system');
  } else if (kind === 'scan' && scan) {
    if (cancelled && scan.new === 0) {
      // Abgebrochen ohne Neues: die Laufleiste genügt.
    } else if (scan.alertMails === 0) {
      choice = await infoBox('Keine Alert-Mails', text('Keine Job-Alert-Mails im gewählten Umfang gefunden.'), withFolder);
    } else if (scan.postingsTotal === 0) {
      choice = await infoBox('Keine Jobs erkannt', text(`${plural(scan.alertMails, 'Alert-Mail', 'Alert-Mails')} gefunden, aber keine Jobs darin – vermutlich hat sich das Mail-Layout geändert.`), withFolder);
    } else if (scan.new > 0) {
      const head = [`${plural(scan.new, 'neuer Job wurde', 'neue Jobs wurden')} gespeichert${cancelled ? ' (Teilergebnis)' : ''}.`];
      if (scan.knownBefore) head.push(`${scan.knownBefore} bereits bekannt, übersprungen.`);
      if (scan.dupInRun) head.push(`${scan.dupInRun} doppelt in mehreren Mails, übersprungen.`);
      if (summary.export?.overview) head.push('', `Datei: ${summary.export.overview}`);
      choice = await infoBox('Abruf abgeschlossen', text(...head), withFolder);
    } else {
      choice = await infoBox('Keine neuen Jobs', text(
        `${plural(scan.knownBefore, 'Job', 'Jobs')} bereits bekannt, ${scan.dupInRun} doppelt in mehreren Mails – alle übersprungen.`,
      ), withFolder);
    }
  } else if (lines.length && !(cancelled && fetchTotals(summary).ok === 0)) {
    const totals = fetchTotals(summary);
    choice = await infoBox(totals.failed || totals.skipped ? 'Jobdetails gespeichert – mit Hinweisen' : 'Jobdetails gespeichert', lines.join('\n'), withFolder);
  }
  if (choice === 'folder') emit('openResults');
  if (summary.export?.error) await errorBox('Ergebnisdateien nicht geschrieben', summary.export.error);
}
