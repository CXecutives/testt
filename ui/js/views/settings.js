// Einstellungen: eine Seite, vier Abschnitte, eine Zeile je Sache.
// Postfach · Portale · Dateien · Wartung.

import { api } from '../api.js';
import { refreshApp, saveSettings, startRun } from '../run.js';
import { addLog, locked, lockedWhy, setSession, state, subscribe } from '../store.js';
import {
  blocker, button, closeDialog, confirmDialog, el, errorDialog, field, midTruncate,
  plural, settingRow, toggle,
} from '../ui.js';
import { flash } from './status.js';

const n = {};
const portalRows = new Map();

export function build(page) {
  n.inner = el('div', { class: 'settings-inner' },
    group('Postfach', mailbox()),
    group('Portale', portals()),
    group('Dateien', files()),
    group('Wartung', maintenance()));
  page.append(el('div', { class: 'settings' }, n.inner));
  subscribe('app', render);
  subscribe('busy', render);
  render();
}

const group = (title, ...body) => el('section', { class: 'group' },
  el('h2', { class: 'group-title', text: title }), ...body);

/* ------------------------------------------------------------------ Postfach */

function mailbox() {
  n.user = field({
    id: 'gmail-user', label: 'Gmail-Adresse', placeholder: 'name@gmail.com',
    onInput: (value) => { state.gmailForm.user = value; render(); },
  });
  n.password = field({
    id: 'gmail-password', label: 'App-Passwort', type: 'password',
    onInput: (value) => { state.gmailForm.password = value; render(); },
  });
  n.save = button({ label: 'Speichern', variant: 'primary', onAction: saveGmail });
  n.create = button({
    label: 'App-Passwort anlegen', icon: 'external', variant: 'ghost',
    onAction: () => api.openTarget({ kind: 'appPasswordPage' }).catch(() => {}),
  });
  n.form = el('div', { class: 'form-grid' }, n.user.node, n.password.node);
  n.formActions = el('div', { class: 'form-actions' }, n.save, n.create);

  n.connected = settingRow({ label: 'Verbunden', hint: '', controls: [] });
  n.change = button({ label: 'Ändern', onClick: () => { n.editing = true; render(); } });
  n.remove = button({ label: 'Entfernen', tone: 'danger', onAction: removeGmail });
  n.connected.node.querySelector('.setting-controls').append(n.change, n.remove);

  n.readOnly = settingRow({ label: 'Zugriff', hint: '' });
  return [n.connected.node, n.form, n.formActions, n.readOnly.node];
}

const canSave = () => Boolean(state.gmailForm.user.trim()) && state.gmailForm.password !== '' && !locked();

async function saveGmail() {
  if (!canSave()) return;
  if (state.app.gmailUser) {
    const ok = await confirmDialog('Zugangsdaten überschreiben', 'Die gespeicherten Zugangsdaten werden ersetzt.', { yes: 'Überschreiben' });
    if (!ok) return;
  }
  try {
    await api.saveGmail(state.gmailForm.user, state.gmailForm.password);
  } catch (error) {
    await errorDialog('Zugangsdaten nicht gespeichert', error.message);
    return;
  }
  state.gmailForm.password = '';
  n.editing = false;
  addLog('ok', 'Zugangsdaten gespeichert.');
  await refreshApp().catch(() => {});
  flash('Zugangsdaten gespeichert');
}

async function removeGmail() {
  const ok = await confirmDialog('Zugangsdaten entfernen',
    'Der nächste Abruf liest dann wieder die letzten sieben Tage.', { yes: 'Entfernen', danger: true });
  if (!ok) return;
  try {
    await api.deleteGmail();
  } catch (error) {
    await errorDialog('Zugangsdaten nicht entfernt', error.message);
    return;
  }
  state.gmailForm = { user: '', password: '' };
  addLog('info', 'Zugangsdaten entfernt.');
  await refreshApp().catch(() => {});
}

/* ------------------------------------------------------------------ Portale */

function portals() {
  n.portals = el('div');
  return [n.portals];
}

function portalRow(portal) {
  const row = settingRow({ label: portal.label, hint: '' });
  const controls = row.node.querySelector('.setting-controls');
  const entry = { row, controls };

  if (portal.login !== 'none') {
    entry.login = button({ label: 'Anmelden', onAction: () => session(portal, true) });
    entry.logout = button({ label: 'Abmelden', onAction: () => session(portal, false) });
    controls.append(entry.login, entry.logout);
  }
  entry.browser = button({
    label: 'Im Browser', icon: 'external', variant: 'ghost',
    onAction: () => api.openTarget({ kind: 'portalHome', portal: portal.portal }).catch(() => {}),
  });
  controls.append(entry.browser);
  entry.toggle = toggle({
    checked: portal.enabled,
    title: 'Beim Abruf berücksichtigen',
    onChange: (on) => {
      const chosen = state.app.settings.portals.filter((p) => p !== portal.portal);
      if (on) chosen.push(portal.portal);
      saveSettings({ portals: chosen });
    },
  });
  controls.append(entry.toggle.node);
  return entry;
}

/** An- und Abmelden sperrt die Oberfläche wie ein Lauf – es ist ein Portalzugriff. */
async function session(portal, signIn) {
  setSession(signIn ? `Anmeldefenster offen – ${portal.label}` : `Abmelden – ${portal.label}`);
  let done;
  try {
    done = signIn ? await api.portalLogin(portal.portal) : await api.portalLogout(portal.portal);
  } catch (error) {
    setSession(null);
    await errorDialog(signIn ? 'Anmeldung nicht möglich' : 'Abmeldung nicht möglich', error.message);
    return;
  } finally {
    setSession(null);
    await refreshApp().catch(() => {});
  }
  if (done) {
    addLog('ok', `${portal.label}: ${signIn ? 'angemeldet' : 'abgemeldet'}.`);
    flash(`${portal.label} ${signIn ? 'angemeldet' : 'abgemeldet'}`);
  } else {
    flash(signIn ? 'Anmeldung nicht abgeschlossen' : 'Abmeldung nicht bestätigt');
  }
}

function portalState(portal) {
  if (portal.pausedUntil) return `Pausiert bis ${short(portal.pausedUntil)}`;
  if (portal.nextFreeAt) return `Obergrenze – weiter ab ${short(portal.nextFreeAt)}`;
  if (portal.login === 'none') return 'Kein Konto nötig';
  if (portal.signedIn === true) return 'Angemeldet';
  if (portal.signedIn === false) return 'Anmeldung nötig';
  return 'Anmeldung ungeprüft';
}

const short = (value) => new Date(value).toLocaleString('de-DE', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });

/* ------------------------------------------------------------------ Dateien */

function files() {
  n.workspace = settingRow({
    label: 'Arbeitsordner',
    hint: '',
    controls: [button({ label: 'Ändern', onAction: chooseWorkspace })],
  });
  n.results = settingRow({
    label: 'Ergebnisse',
    hint: '',
    controls: [button({
      label: 'Öffnen', icon: 'folder', variant: 'ghost',
      onAction: () => api.openTarget({ kind: 'resultFolder' }).catch(() => {}),
    })],
  });
  n.txtClear = button({ label: 'Leeren', tone: 'danger', onAction: clearTxt });
  n.txtWrite = button({ label: 'Neu schreiben', onAction: rewriteTxt });
  n.txt = settingRow({ label: 'Textdateien', hint: '', controls: [n.txtClear, n.txtWrite] });
  n.profileUp = button({ label: 'Wählen', onAction: pickProfile });
  n.profileDel = button({ label: 'Entfernen', tone: 'danger', onAction: removeProfile });
  n.profile = settingRow({ label: 'Beraterprofil', hint: '', controls: [n.profileUp, n.profileDel] });
  return [n.workspace.node, n.results.node, n.txt.node, n.profile.node];
}

async function chooseWorkspace() {
  const before = state.app.workspace;
  let picked;
  try {
    picked = await api.pickWorkspace();
  } catch (error) {
    await errorDialog('Ordner nicht gewählt', error.message);
    return;
  }
  if (picked === null || picked.toLowerCase() === before.toLowerCase()) return;
  await refreshApp().catch(() => {});
  flash('Arbeitsordner geändert');
}

async function rewriteTxt() {
  let summary;
  try {
    summary = await api.rewriteTxt();
  } catch (error) {
    await errorDialog('Textdateien nicht geschrieben', error.message);
    return;
  }
  await refreshApp().catch(() => {});
  if (summary.error) await errorDialog('Textdateien nicht vollständig geschrieben', summary.error);
  else flash(`${plural(summary.txtWritten, 'Textdatei', 'Textdateien')} geschrieben`);
}

async function clearTxt() {
  const count = state.app.txtFiles;
  const ok = await confirmDialog('Textdateien leeren',
    `${plural(count, 'Datei wird', 'Dateien werden')} gelöscht. Die Übersicht und die gespeicherten Jobs bleiben.`,
    { yes: 'Leeren', danger: true });
  if (!ok) return;
  try {
    const result = await api.clearTxtFiles();
    await refreshApp().catch(() => {});
    flash(`${plural(result.removed, 'Datei', 'Dateien')} gelöscht`);
  } catch (error) {
    await errorDialog('Textdateien nicht gelöscht', error.message);
  }
}

async function pickProfile() {
  let profile;
  try {
    profile = await api.pickProfile();
  } catch (error) {
    await errorDialog('Profil nicht gespeichert', error.message);
    return;
  }
  if (profile === null) return;
  await refreshApp().catch(() => {});
  flash('Beraterprofil gespeichert');
}

async function removeProfile() {
  const ok = await confirmDialog('Beraterprofil entfernen', 'Die Datei wird gelöscht.', { yes: 'Entfernen', danger: true });
  if (!ok) return;
  try {
    await api.removeProfile();
  } catch (error) {
    await errorDialog('Profil nicht entfernt', error.message);
    return;
  }
  await refreshApp().catch(() => {});
}

/* ------------------------------------------------------------------ Wartung */

function maintenance() {
  n.full = button({ label: 'Durchsuchen', onAction: () => startRun('all') });
  n.log = button({
    label: 'Öffnen', variant: 'ghost', icon: 'folder',
    onAction: () => api.openTarget({ kind: 'logFolder' }).catch(() => {}),
  });
  n.reset = button({ label: 'Zurücksetzen', tone: 'danger', onAction: resetAll });
  return [
    settingRow({ label: 'Ganzes Postfach durchsuchen', hint: 'Sonst liest die App ab dem letzten Lauf.', controls: [n.full] }).node,
    settingRow({ label: 'Protokoll', hint: '', controls: [n.log] }).node,
    settingRow({ label: 'Alles zurücksetzen', hint: 'Löscht Jobs, Zugang, Anmeldungen und die Dateien der App.', controls: [n.reset] }).node,
  ];
}

async function resetAll() {
  const ok = await confirmDialog('Alles zurücksetzen',
    'Jobs, Zugangsdaten, Portal-Anmeldungen, Beraterprofil und die Dateien der App werden gelöscht. Die App startet danach neu.',
    { yes: 'Zurücksetzen', danger: true });
  if (!ok || locked()) return;
  blocker('reset', 'Wird zurückgesetzt …', 'Die App startet gleich neu.');
  try {
    await api.resetAll();
  } catch (error) {
    closeDialog('reset');
    await errorDialog('Zurücksetzen nicht möglich', error.message);
  }
}

/* ------------------------------------------------------------------ Anzeige */

function render() {
  const app = state.app;
  if (!app) return;
  const why = lockedWhy();
  const saved = Boolean(app.gmailUser);
  const editing = n.editing || !saved;

  // Postfach
  n.connected.node.hidden = editing;
  n.form.hidden = !editing;
  n.formActions.hidden = !editing;
  n.connected.setHint(app.gmailUser ?? '');
  n.readOnly.setHint(`Nur lesend. Das Passwort liegt im ${app.vaultName}.`);
  if (n.user.input.value !== state.gmailForm.user) n.user.input.value = state.gmailForm.user;
  n.user.setDisabled(Boolean(why));
  n.password.setDisabled(Boolean(why));
  n.password.input.placeholder = saved ? '•'.repeat(16) : '16 Buchstaben';
  n.save.disabled = !canSave();
  n.save.title = why ?? (canSave() ? '' : 'Adresse und App-Passwort eingeben');
  n.change.disabled = Boolean(why);
  n.change.title = why ?? '';
  n.remove.disabled = Boolean(why);
  n.remove.title = why ?? '';

  // Portale
  const nodes = app.portals.map((portal) => {
    let entry = portalRows.get(portal.portal);
    if (!entry) { entry = portalRow(portal); portalRows.set(portal.portal, entry); }
    entry.row.setHint(portalState(portal));
    entry.toggle.set(portal.enabled);
    entry.toggle.setDisabled(Boolean(why));
    entry.browser.hidden = !portal.pausedUntil;
    if (entry.login) {
      const busy = why ?? (portal.pausedUntil ? 'Portal pausiert' : null);
      entry.login.disabled = Boolean(busy) || portal.signedIn === true;
      entry.login.title = busy ?? (portal.signedIn === true ? 'Bereits angemeldet' : '');
      entry.logout.disabled = Boolean(busy) || portal.signedIn !== true;
      entry.logout.title = busy ?? (portal.signedIn === true ? '' : 'Keine bestätigte Anmeldung');
    }
    return entry.row.node;
  });
  n.portals.replaceChildren(...nodes);

  // Dateien
  n.workspace.setHint(midTruncate(app.workspace));
  n.results.setHint(midTruncate(app.resultDir));
  n.txt.setHint(`${plural(app.txtFiles, 'Datei', 'Dateien')} im Ordner`);
  n.txtClear.disabled = Boolean(why) || app.txtFiles === 0;
  n.txtClear.title = why ?? (app.txtFiles ? '' : 'Keine Textdateien vorhanden');
  n.txtWrite.disabled = Boolean(why) || app.jobsTotal === 0;
  n.txtWrite.title = why ?? (app.jobsTotal ? '' : 'Noch keine Jobs gespeichert');
  n.profile.setHint(app.profileError ?? (app.profile ? midTruncate(app.profile.path) : 'Keines hinterlegt'));
  n.profileUp.disabled = Boolean(why);
  n.profileUp.title = why ?? '';
  n.profileDel.disabled = Boolean(why) || !app.profile;
  n.profileDel.title = why ?? (app.profile ? '' : 'Keines hinterlegt');

  // Wartung
  n.full.disabled = Boolean(why) || !saved;
  n.full.title = why ?? (saved ? '' : 'Erst das Postfach verbinden');
  n.reset.disabled = Boolean(why) || app.dryRun;
  n.reset.title = why ?? (app.dryRun ? 'Im Trockenlauf wird nichts zurückgesetzt' : '');
}
