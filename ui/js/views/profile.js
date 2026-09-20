// Ansicht „Beraterprofil“: JSON-Profil hochladen, anzeigen, entfernen.

import { api } from '../api.js';
import { openTarget } from '../actions.js';
import { refreshApp } from '../run.js';
import { addLog, locked, state, subscribe } from '../store.js';
import { action, confirmBox, el, errorBox, formatBytes, formatTime, icon, toast } from '../ui.js';

const n = {};

export function build(section) {
  n.statusIcon = el('span');
  n.statusText = el('span');
  n.status = el('div', { class: 'note' }, n.statusIcon, n.statusText);
  n.upload = el('button', { class: 'btn primary', type: 'button', text: 'Profil hochladen…' });
  n.folder = el('button', {
    class: 'btn', type: 'button', text: 'Ordner öffnen',
    onclick: () => openTarget({ kind: 'profileFolder' }, { errorTitle: 'Ordner nicht geöffnet', missingTitle: 'Beraterprofil fehlt' }),
  });
  n.remove = el('button', { class: 'btn danger', type: 'button', text: 'Profil entfernen…' });
  action(n.upload, upload);
  action(n.remove, remove);
  section.append(el('div', { class: 'narrow' }, el('div', { class: 'card' },
    el('div', { class: 'card-head' },
      el('div', { class: 'card-title' },
        el('h2', { text: 'Profildatei' }),
        el('p', { text: 'Bleibt lokal auf diesem Rechner' }))),
    el('div', { class: 'card-body' },
      n.status,
      el('p', { class: 'hint', text: 'Eine JSON-Datei; ein vorhandenes Profil wird ersetzt.' })),
    el('div', { class: 'card-foot' }, n.upload, n.folder, el('span', { class: 'spacer' }), n.remove))));
  subscribe('app', render);
  subscribe('busy', render);
  render();
}

const fileName = (path) => path.split(/[\\/]/).pop();

function setStatus(level, text) {
  n.status.className = `note ${level}`.trim();
  n.statusIcon.replaceChildren(icon(level || 'info'));
  n.statusText.textContent = text;
}

function render() {
  const app = state.app;
  const profile = app.profile;
  if (app.profileError) setStatus('error', `Profil nicht lesbar: ${app.profileError}`);
  else if (!profile) setStatus('', `Kein Profil hinterlegt.\nSpeicherort: ${app.profileDir}`);
  else if (profile.parseError) setStatus('error', `Datei beschädigt (kein gültiges JSON)\n${profile.path}\n${profile.parseError}`);
  else setStatus('ok', `${describe(profile)}\n${profile.path}`);
  const saved = Boolean(profile);
  // Jeder gesperrte Knopf sagt im Tooltip, warum – nie ein stummes „grau“.
  const busy = locked() ? 'Erst nach dem laufenden Vorgang möglich' : null;
  n.upload.disabled = Boolean(busy);
  n.upload.title = busy ?? '';
  n.remove.disabled = Boolean(busy) || !saved;
  n.remove.title = busy ?? (saved ? '' : 'Kein Profil hinterlegt');
  n.folder.disabled = !saved;
  n.folder.title = saved ? '' : 'Der Ordner entsteht beim Hochladen';
}

function describe(profile) {
  return [fileName(profile.path), formatBytes(profile.bytes), formatTime(profile.savedAt), profile.content]
    .filter(Boolean).join(' · ');
}

async function upload(busy) {
  let profile;
  try {
    profile = await busy(api.pickProfile());
  } catch (error) {
    await errorBox('Profil nicht gespeichert', error.message);
    return;
  }
  if (profile === null) return;
  addLog('ok', `Beraterprofil gespeichert: ${profile.path} (${profile.content}).`);
  await refreshApp().catch(() => {});
  // Erfolg meldet die Oberfläche überall als kurze Meldung, nur Fehler halten den Nutzer auf.
  toast('Beraterprofil gespeichert.', 'ok');
}

async function remove(busy) {
  const ok = await confirmBox('Beraterprofil entfernen', 'Die Datei „beraterprofil.json“ wird gelöscht.', { yes: 'Entfernen', danger: true });
  if (!ok) return;
  try {
    await busy(api.removeProfile());
  } catch (error) {
    await errorBox('Profil nicht entfernt', error.message);
    return;
  }
  addLog('info', 'Beraterprofil entfernt.');
  await refreshApp().catch(() => {});
}
