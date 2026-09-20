// Aktionen, die mehrere Ansichten anbieten – mit genau einer Meldung je Ergebnis.

import { api } from './api.js';
import { refreshApp, saveSettings } from './run.js';
import { addLog, state } from './store.js';
import { bullets, confirmBox, errorBox, infoBox, modal, plural, toast, warnBox } from './ui.js';

const FIRST_RUN = 'Die App liest Ihren Gmail-Posteingang nach Job-Alert-Mails von LinkedIn, freelance.de und freelancermap.de durch – nur lesend: sie ändert, löscht und versendet nichts.\n\n'
  + '• Gmail braucht dafür ein App-Passwort, nicht Ihr Kontopasswort. Es liegt im Windows-Tresor, nie im Klartext.\n\n'
  + '• Nur freelance.de verlangt eine Anmeldung – einmal selbst im Fenster der App, danach läuft es automatisch.\n\n'
  + '• Die Portale werden mit Pausen und Obergrenzen abgerufen; ein überlastetes Portal pausiert die App von selbst.';

/** Erststart-Hinweis; danach gilt er als gelesen. */
export async function showFirstRunNotice() {
  await modal({
    title: 'Willkommen',
    body: FIRST_RUN,
    buttons: [{ label: 'Verstanden', value: true, primary: true }],
  });
  if (!state.app.settings.firstRunSeen) saveSettings({ firstRunSeen: true });
}

// Dasselbe Ziel binnen einer Sekunde nur einmal öffnen (Doppel- und Mehrfachklicks).
let lastOpen = { key: '', at: 0 };

/** Ziel öffnen (Anzeige, Mail, Ordner …). „Gibt es nicht“ ist ein Hinweis, kein Fehler. */
export async function openTarget(target, { errorTitle = 'Seite ließ sich nicht öffnen', missingTitle = null } = {}) {
  const key = JSON.stringify(target);
  const now = performance.now();
  if (key === lastOpen.key && now - lastOpen.at < 1000) return;
  lastOpen = { key, at: now };
  try {
    await api.openTarget(target);
  } catch (error) {
    if (error.kind === 'notFound' && missingTitle) await infoBox(missingTitle, error.message);
    else await errorBox(errorTitle, error.message);
  }
}

export const openResultFolder = () =>
  openTarget({ kind: 'resultFolder' }, { errorTitle: 'Ordner nicht geöffnet', missingTitle: 'Ergebnisordner fehlt' });

/** Arbeitsordner per Dialog wählen; danach bei Bedarf die Textdateien neu schreiben. */
export async function chooseWorkspace(busy) {
  const before = state.app.workspace;
  let picked;
  try {
    picked = await busy(api.pickWorkspace());
  } catch (error) {
    await errorBox('Ordner nicht gewählt', error.message);
    return;
  }
  if (picked === null) return;
  // Derselbe Ordner noch einmal gewählt: nichts geändert, keine Rückfrage.
  if (picked.toLowerCase() === before.toLowerCase()) {
    toast('Arbeitsordner unverändert.');
    return;
  }
  await refreshApp().catch((error) => addLog('error', `Zustand nicht geladen: ${error.message}`));
  // Eine Meldung, ein Kanal: Folgt die Rückfrage, steht die Bestätigung darin.
  if (!state.app.jobsTotal) {
    toast('Arbeitsordner gesetzt.', 'ok');
    return;
  }
  const again = await confirmBox(
    'Arbeitsordner geändert',
    `Neue Ergebnisse landen jetzt in\n${state.app.resultDir}.\n\nDie Textdateien bisheriger Jobs liegen noch im alten Ordner. Jetzt hier neu schreiben?`,
    { yes: 'Neu schreiben' },
  );
  if (again) await writeTexts(busy);
}

/** „Textdateien neu schreiben…“ mit Rückfrage. */
export async function rewriteTexts(busy) {
  const ok = await confirmBox(
    'Textdateien neu schreiben',
    `Alle gespeicherten Jobdetails werden erneut nach\n${state.app.txtDir}\ngeschrieben; vorhandene Dateien werden ersetzt.\n\n`
      + 'Nötig nur nach einem Ordnerwechsel oder nach „Ergebnisordner leeren“.',
    { yes: 'Neu schreiben' },
  );
  if (ok) await writeTexts(busy);
}

async function writeTexts(busy) {
  let summary;
  try {
    summary = await busy(api.rewriteTxt());
  } catch (error) {
    await errorBox('Textdateien nicht geschrieben', error.message);
    return;
  }
  const failed = summary.txtFailedCount || summary.error;
  const lines = [`${plural(summary.txtWritten, 'Textdatei', 'Textdateien')} geschrieben.`];
  if (summary.txtFailedCount) {
    // Die Namensliste ist gekappt; die Zahl stimmt.
    const names = summary.txtFailed.length < summary.txtFailedCount ? [...summary.txtFailed, '…'] : summary.txtFailed;
    lines.push(`${plural(summary.txtFailedCount, 'Textdatei ließ', 'Textdateien ließen')} sich nicht schreiben:\n${bullets(names)}`);
  }
  if (summary.error) lines.push(summary.error);
  addLog(failed ? 'warn' : 'ok', `Textdateien neu geschrieben: ${summary.txtWritten}.`);
  await refreshApp().catch(() => {});
  if (failed) await warnBox('Textdateien nicht vollständig geschrieben', lines.join('\n\n'));
  else await infoBox('Textdateien geschrieben', lines.join('\n\n'));
}
