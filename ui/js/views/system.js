// Ansicht „System“: Gmail-Zugang, Speicherorte, Ergebnisse, Über diese App, Zurücksetzen.
// Alles wird einmal gebaut und danach nur aktualisiert.

import { api } from '../api.js';
import { chooseWorkspace, openResultFolder, openTarget, rewriteTexts, showFirstRunNotice } from '../actions.js';
import { refreshApp } from '../run.js';
import { addLog, locked, lockedWhy, state, subscribe } from '../store.js';
import {
  action, blocker, bullets, closeModal, confirmBox, el, errorBox, icon, modal, plural, toast, warnBox,
} from '../ui.js';

const n = {};

export function build(section) {
  section.append(el('div', { class: 'system-grid' },
    el('div', { class: 'stack' }, gmailCard(), placesCard(), resultsCard()),
    el('div', { class: 'stack' }, aboutCard(), resetCard())));
  subscribe('app', render);
  subscribe('busy', render);
  render();
}

function render() {
  renderGmail();
  renderPlaces();
}

/** Schlüssel-Wert-Liste mit festen Zeilen; liefert die dd-Knoten zum Aktualisieren. */
function kvList(names) {
  const dl = el('dl', { class: 'kv' });
  const dd = {};
  for (const name of names) {
    dd[name] = el('dd');
    dl.append(el('dt', { text: name }), dd[name]);
  }
  return [dl, dd];
}

function note() {
  const text = el('span');
  const box = el('div', { class: 'note' }, el('span'), text);
  return (level, value) => {
    box.className = `note ${level}`.trim();
    box.firstChild.replaceChildren(icon(level || 'info'));
    text.textContent = value;
    return box;
  };
}

const card = (title, sub, body, foot) => el('div', { class: 'card' },
  el('div', { class: 'card-head' }, el('div', { class: 'card-title' }, el('h2', { text: title }), sub ? el('p', { text: sub }) : null)),
  el('div', { class: 'card-body' }, body),
  foot ? el('div', { class: 'card-foot' }, foot) : null);

/* ------------------------------------------------------------------ Gmail */

function gmailCard() {
  n.user = el('input', {
    type: 'text', id: 'gmail-user', placeholder: 'name@gmail.com', autocomplete: 'off', spellcheck: 'false',
    oninput: () => {
      state.gmailForm.user = n.user.value;
      renderGmail();
    },
  });
  n.password = el('input', {
    type: 'password', id: 'gmail-password', autocomplete: 'off',
    oninput: () => {
      state.gmailForm.password = n.password.value;
      renderGmail();
    },
    onkeydown: (event) => {
      if (event.key !== 'Enter') return;
      // Sonst träfe dasselbe Enter den Knopf der folgenden Rückfrage.
      event.preventDefault();
      if (canSave()) n.runSave();
    },
  });
  n.passwordHint = el('p', { class: 'hint' });
  n.gmailNote = note();
  n.save = el('button', { class: 'btn primary', type: 'button', text: 'Speichern' });
  n.discard = el('button', { class: 'btn', type: 'button', text: 'Verwerfen', onclick: discardGmail });
  n.create = el('button', {
    class: 'btn', type: 'button', text: 'App-Passwort anlegen…',
    onclick: () => openTarget({ kind: 'appPasswordPage' }),
  });
  n.remove = el('button', { class: 'btn danger', type: 'button', text: 'Entfernen…' });
  n.runSave = action(n.save, saveGmail);
  action(n.remove, removeGmail);
  return card('Gmail-Zugang', 'App-Passwort von Google, nicht das Kontopasswort', [
    el('div', { class: 'form-grid' },
      el('div', { class: 'field' }, el('label', { class: 'field-label', for: 'gmail-user', text: 'Gmail-Adresse' }), n.user),
      el('div', { class: 'field' }, el('label', { class: 'field-label', for: 'gmail-password', text: 'App-Passwort' }), n.password, n.passwordHint)),
    n.gmailNote('', ''),
    el('p', { class: 'hint', text: 'Gespeichert im Windows-Tresor, nie im Klartext.' }),
  ], [n.save, n.discard, n.create, el('span', { class: 'spacer' }), n.remove]);
}

const savedUser = () => state.app.gmailUser ?? null;

/** Weicht die Eingabe vom Gespeicherten ab? */
function gmailDirty() {
  const { user, password } = state.gmailForm;
  return (user ?? '').trim().toLowerCase() !== (savedUser() ?? '') || password !== '';
}

const canSave = () => Boolean(state.gmailForm.user?.trim()) && state.gmailForm.password !== '' && !locked();

function renderGmail() {
  const saved = savedUser() !== null;
  const typed = state.gmailForm.password !== '';
  if (n.user.value !== state.gmailForm.user) n.user.value = state.gmailForm.user ?? '';
  if (n.password.value !== state.gmailForm.password) n.password.value = state.gmailForm.password;
  n.user.disabled = locked();
  n.password.disabled = locked();
  n.password.placeholder = saved && !typed ? '•'.repeat(16) : '16-stelliges App-Passwort';
  // Nie leer: ein verschwindender Hinweis würde beim Tippen alle Knöpfe darunter verschieben.
  n.passwordHint.textContent = saved && !typed ? 'Zum Ändern überschreiben.' : 'Leerzeichen werden entfernt.';
  if (state.app.gmailError) n.gmailNote('warn', state.app.gmailError);
  else if (!saved) n.gmailNote('', 'Noch keine Zugangsdaten gespeichert.');
  else n.gmailNote('ok', `Zugangsdaten gespeichert (${savedUser()}).`);
  const busy = lockedWhy();
  n.save.disabled = !canSave();
  n.save.title = busy ?? (canSave() ? '' : saved && !gmailDirty() ? 'Nichts geändert' : 'Adresse und App-Passwort eingeben');
  n.discard.disabled = Boolean(busy) || !gmailDirty();
  n.discard.title = busy ?? (gmailDirty() ? 'Das gespeicherte Passwort bleibt erhalten' : 'Nichts geändert');
  n.remove.disabled = Boolean(busy) || !saved;
  n.remove.title = busy ?? (saved ? '' : 'Keine Zugangsdaten gespeichert');
}

/** Speichert die Eingabe; true bei Erfolg. */
async function saveGmail(busy) {
  if (!canSave()) return false;
  if (savedUser() !== null) {
    const ok = await confirmBox('Zugangsdaten aktualisieren', 'Die gespeicherten Zugangsdaten werden überschrieben.',
      { yes: 'Aktualisieren' });
    if (!ok) return false;
  }
  let user;
  try {
    user = await busy(api.saveGmail(state.gmailForm.user, state.gmailForm.password));
  } catch (error) {
    await errorBox('Zugangsdaten nicht gespeichert', error.message);
    return false;
  }
  state.gmailForm.user = user;
  state.gmailForm.password = '';
  addLog('ok', 'Gmail-Zugangsdaten im Windows-Tresor gespeichert.');
  await refreshApp().catch(() => {});
  toast('Gmail-Zugangsdaten gespeichert.', 'ok');
  return true;
}

/** Eingabe auf den gespeicherten Stand zurücksetzen. */
function discardGmail() {
  const saved = savedUser() !== null;
  state.gmailForm.user = savedUser() ?? '';
  state.gmailForm.password = '';
  renderGmail();
  toast(saved ? 'Änderung verworfen – die gespeicherten Zugangsdaten bleiben.' : 'Eingaben verworfen.');
}

async function removeGmail(busy) {
  const ok = await confirmBox('Zugangsdaten entfernen', 'Die Zugangsdaten werden aus dem Windows-Tresor entfernt.\n\n'
    + 'Der nächste Abruf mit „Neu seit letztem Lauf“ liest dann wieder die letzten 7 Tage.', { yes: 'Entfernen', danger: true });
  if (!ok) return;
  try {
    await busy(api.deleteGmail());
  } catch (error) {
    await errorBox('Zugangsdaten nicht entfernt', error.message);
    return;
  }
  state.gmailForm.user = '';
  state.gmailForm.password = '';
  addLog('info', 'Gmail-Zugangsdaten entfernt.');
  await refreshApp().catch(() => {});
}

/** Beim Verlassen der Ansicht: ungespeicherte Eingaben klären. false = hierbleiben. */
export async function confirmLeave() {
  if (!gmailDirty()) return true;
  const choice = await modal({
    title: 'Ungespeicherte Zugangsdaten',
    kind: 'warn',
    icon: 'question',
    stale: () => !gmailDirty() || state.view !== 'system',
    body: 'Die Gmail-Zugangsdaten wurden geändert, aber nicht gespeichert.',
    buttons: [
      { label: 'Speichern', value: 'save', primary: true },
      { label: 'Verwerfen', value: 'discard' },
      { label: 'Hierbleiben', value: null },
    ],
  });
  if (choice === 'save') {
    if (canSave()) return n.runSave();
    await errorBox('Zugangsdaten nicht gespeichert', locked()
      ? 'Während eines Laufs oder einer Anmeldung lassen sich die Zugangsdaten nicht speichern.'
      : !state.gmailForm.user?.trim()
        ? 'Bitte die Gmail-Adresse eingeben.'
        : 'Zum Speichern bitte auch das App-Passwort eingeben.');
    return false;
  }
  if (choice === 'discard') {
    discardGmail();
    return true;
  }
  return !gmailDirty();
}

/* ------------------------------------------------------------------ Speicherorte und Ergebnisse */

function placesCard() {
  [n.places, n.placeDd] = kvList(['Arbeitsordner', 'Ergebnisordner', 'Beraterprofil', 'Programmdaten']);
  n.pick = el('button', { class: 'btn', type: 'button', text: 'Arbeitsordner wählen…' });
  action(n.pick, chooseWorkspace);
  return card('Speicherorte', null, n.places, [
    n.pick,
    el('button', { class: 'btn', type: 'button', text: 'Ergebnisordner öffnen', onclick: openResultFolder }),
    el('button', {
      class: 'btn', type: 'button', text: 'Protokoll öffnen',
      onclick: () => openTarget({ kind: 'logFolder' }, { errorTitle: 'Ordner nicht geöffnet', missingTitle: 'Protokoll fehlt' }),
    }),
  ]);
}

function resultsCard() {
  n.resultNote = note();
  n.rewrite = el('button', { class: 'btn', type: 'button', text: 'Textdateien neu schreiben…' });
  n.clear = el('button', { class: 'btn danger', type: 'button', text: 'Ergebnisordner leeren…' });
  action(n.rewrite, rewriteTexts);
  action(n.clear, clearResults);
  return card('Ergebnisse', 'JobAlerts.xlsx/.csv und Textdateien', n.resultNote('', ''),
    [n.rewrite, el('span', { class: 'spacer' }), n.clear]);
}

function aboutCard() {
  [n.about, n.aboutDd] = kvList(['WebView2-Runtime', 'Betriebsart', 'Gespeicherte Jobs']);
  return card('Über diese App', null, n.about,
    [el('button', { class: 'btn', type: 'button', text: 'Hinweis anzeigen', onclick: showFirstRunNotice })]);
}

function resetCard() {
  n.reset = el('button', { class: 'btn danger', type: 'button', text: 'Zurücksetzen…' });
  action(n.reset, resetAll);
  const warn = note();
  return card('Alles zurücksetzen', null,
    warn('warn', 'Löscht Gmail-Zugang, gesammelte Jobs, freelance.de-Anmeldung, Beraterprofil und die Dateien der App im Ergebnisordner.'),
    [el('span', { class: 'spacer' }), n.reset]);
}

function renderPlaces() {
  const app = state.app;
  n.placeDd.Arbeitsordner.textContent = app.workspace;
  n.placeDd.Ergebnisordner.textContent = app.resultDir;
  n.placeDd.Beraterprofil.textContent = app.profileDir;
  n.placeDd.Programmdaten.textContent = app.dataDir;
  const busy = lockedWhy();
  n.pick.disabled = Boolean(busy);
  n.pick.title = busy ?? '';
  n.resultNote('', app.resultDirExists
    ? `${plural(app.resultFiles, 'Datei', 'Dateien')} der App im Ergebnisordner.`
    : 'Der Ergebnisordner entsteht beim ersten Lauf.');
  n.clear.disabled = Boolean(busy) || app.resultFiles === 0;
  n.clear.title = busy ?? (app.resultFiles ? '' : 'Keine Dateien der App im Ergebnisordner');
  n.rewrite.disabled = Boolean(busy) || app.jobsTotal === 0;
  n.rewrite.title = busy ?? (app.jobsTotal ? '' : 'Noch keine Jobs gespeichert');
  n.aboutDd['WebView2-Runtime'].textContent = app.webviewVersion || 'unbekannt';
  n.aboutDd.Betriebsart.textContent = app.dryRun ? 'Trockenlauf' : 'Normalbetrieb';
  n.aboutDd['Gespeicherte Jobs'].textContent = String(app.jobsTotal);
  n.reset.disabled = Boolean(busy) || app.dryRun;
  n.reset.title = busy ?? (app.dryRun ? 'Im Trockenlauf wird nichts zurückgesetzt' : '');
}

async function clearResults(busy) {
  const app = state.app;
  const ok = await confirmBox('Ergebnisordner leeren',
    `${plural(app.resultFiles, 'Datei der App wird', 'Dateien der App werden')} gelöscht: JobAlerts.xlsx/.csv und die Textdateien in „beschreibungen_txt“.\n`
    + `${app.resultDir}\n\n`
    + 'Fremde Dateien bleiben unberührt. Die gesammelten Jobs bleiben gespeichert – „Textdateien neu schreiben“ erzeugt die Texte jederzeit wieder.',
    { yes: 'Dateien löschen', danger: true });
  if (!ok) return;
  let result;
  try {
    result = await busy(api.clearResultFiles());
  } catch (error) {
    await errorBox('Ordnerinhalt nicht gelöscht', error.message);
    return;
  }
  await refreshApp().catch(() => {});
  if (!result.failed.length) {
    addLog('info', `Ergebnisordner geleert: ${plural(result.removed, 'Datei', 'Dateien')} gelöscht.`);
    toast('Ergebnisordner geleert.', 'ok');
  } else {
    addLog('warn', `Ergebnisordner teilweise geleert: ${result.removed} gelöscht, ${result.failed.length} nicht.`);
    await warnBox('Ordnerinhalt nicht vollständig gelöscht', `Diese Dateien ließen sich nicht löschen:\n\n${bullets(result.failed)}\n\n`
      + 'Meist sind sie noch in einem Programm geöffnet.');
  }
}

async function resetAll() {
  const ok = await confirmBox('Alles zurücksetzen', 'Unwiderruflich gelöscht werden: alle gesammelten Jobs und Einstellungen der App, '
    + 'der Gmail-Zugang im Windows-Tresor, die freelance.de-Anmeldung und im Arbeitsordner die Dateien der App '
    + '(JobAlerts.xlsx/.csv, „beschreibungen_txt“, Beraterprofil).\n'
    + `${state.app.workspace}\n\n`
    + 'Fremde Dateien bleiben unberührt. Pausen und Abrufzähler der Portale bleiben erhalten. Die App startet danach neu.',
  { yes: 'Zurücksetzen', danger: true });
  if (!ok || locked()) return;
  blocker('resetting', 'Wird zurückgesetzt …', 'Die App startet gleich neu.');
  try {
    await api.resetAll();
  } catch (error) {
    closeModal('resetting');
    await errorBox('Zurücksetzen nicht möglich', error.message);
  }
}

