// Ansicht „Portal-Zugänge“: Abrufstand, Pausen und Kontingente je Portal; freelance.de-Anmeldung.
// Die Karten werden einmal gebaut und danach nur aktualisiert (Knöpfe bleiben unter dem Zeiger).

import { api } from '../api.js';
import { openTarget } from '../actions.js';
import { refreshApp } from '../run.js';
import { addLog, locked, setSession, state, subscribe } from '../store.js';
import { action, closeModal, el, errorBox, formatTime, icon, toast } from '../ui.js';

const cards = new Map();
let timer = 0;

export function build(section) {
  const grid = el('div', { class: 'grid-cards' });
  for (const p of state.app.portals) {
    const card = portalCard(p);
    cards.set(p.portal, card);
    grid.append(card.root);
  }
  section.append(grid);
  subscribe('app', render);
  subscribe('busy', render);
  let runSig = '';
  subscribe('run', (run) => {
    const sig = JSON.stringify([run.stops, run.loginWaiting]);
    if (sig === runSig) return;
    runSig = sig;
    render();
  });
  // Pausen, Obergrenzen und Zähler laufen ab – solange die Ansicht sichtbar ist, minütlich neu.
  subscribe('view', (view) => {
    clearInterval(timer);
    if (view !== 'portals') return;
    if (!locked()) refreshApp().catch(() => {});
    timer = setInterval(() => {
      if (document.hidden) return;
      if (locked()) render();
      else refreshApp().catch(() => render());
    }, 60_000);
  });
  render();
}

function portalCard(p) {
  const c = { root: null };
  c.badge = el('span', { class: 'badge' });
  c.notes = el('div', { class: 'portal-notes' });
  c.kv = el('dl', { class: 'kv' });
  c.dd = {};
  const rows = [...(p.needsAccount ? ['Konto'] : []), 'Letzte Stunde', 'Letzte 24 h', 'Jobdetails'];
  for (const name of rows) {
    c.dd[name] = el('dd');
    c.kv.append(el('dt', { text: name }), c.dd[name]);
  }
  c.browser = el('button', {
    class: 'btn', type: 'button', text: 'Im Browser öffnen',
    title: 'Zum Beispiel für eine Sicherheitsprüfung',
    onclick: () => openTarget({ kind: 'portalHome', portal: p.portal }),
  });
  const foot = el('div', { class: 'card-foot' });
  if (p.needsAccount) {
    c.login = el('button', { class: 'btn primary', type: 'button', text: 'Anmelden…' });
    c.logout = el('button', { class: 'btn', type: 'button', text: 'Abmelden' });
    action(c.login, login);
    action(c.logout, logout);
    foot.append(c.login, c.logout);
  }
  // Fußreihenfolge überall gleich: primär, sekundär, Abstand, zerstörerisch – hier gibt es nichts Zerstörerisches.
  foot.append(c.browser);
  c.foot = foot;
  c.root = el('div', { class: 'card' },
    el('div', { class: 'card-head' },
      el('div', { class: 'card-title' },
        el('h2', { text: p.label }),
        el('p', { text: p.needsAccount ? 'Einmal anmelden, danach automatisch' : 'Kein Konto nötig' })),
      c.badge),
    el('div', { class: 'card-body' }, c.notes, c.kv),
    foot);
  return c;
}

function render() {
  for (const p of state.app.portals) renderCard(cards.get(p.portal), p);
}

function renderCard(c, p) {
  const now = Date.now();
  const paused = Boolean(p.pausedUntil) && Date.parse(p.pausedUntil) > now;
  const blocked = paused && p.pauseKind === 'blocked';
  const capped = Boolean(p.nextFreeAt) && Date.parse(p.nextFreeAt) > now;
  // Stopps gelten nur für den laufenden Lauf.
  const stop = state.busy ? state.run.stops[p.portal] : null;

  const [badgeText, badgeLevel] = blocked ? ['gesperrt', 'error']
    : paused ? ['pausiert', 'warn']
      : capped ? ['Obergrenze', 'warn']
        : p.needsAccount && (p.loginNeeded || waiting(p)) ? ['Anmeldung nötig', 'warn']
          : ['bereit', 'ok'];
  c.badge.className = `badge ${badgeLevel}`;
  c.badge.textContent = badgeText;

  const notes = [];
  if (blocked) notes.push(['error', `Gesperrt bis ${formatTime(p.pausedUntil)} (${p.pauseReason}).`]);
  else if (paused) notes.push(['warn', `Pause bis ${formatTime(p.pausedUntil)} (${p.pauseReason}).`]);
  if (capped) notes.push(['warn', `Obergrenze erreicht – weitere Abrufe ab ${formatTime(p.nextFreeAt)} möglich.`]);
  if (stop) notes.push(['warn', stop.text]);
  const sig = JSON.stringify(notes);
  if (c.notesSig !== sig) {
    c.notesSig = sig;
    c.notes.replaceChildren(...notes.map(([level, text]) => el('div', { class: `note ${level}` }, icon(level), el('span', { text }))));
    c.notes.hidden = !notes.length;
  }

  if (c.dd.Konto) c.dd.Konto.textContent = accountText(p);
  c.dd['Letzte Stunde'].textContent = `${p.usedHour} von ${p.capHour} Abrufen`;
  c.dd['Letzte 24 h'].textContent = `${p.usedDay} von ${p.capDay} Abrufen`;
  c.dd.Jobdetails.textContent = [
    `${p.ok} vorhanden${p.short ? ` (davon ${p.short} kurz)` : ''}`,
    p.open && `${p.open} offen`,
    p.failed && `${p.failed} fehlgeschlagen`,
    p.unfetchable && `${p.unfetchable} nicht abrufbar`,
    p.gone && `${p.gone} abgelaufen`,
  ].filter(Boolean).join(' · ');

  c.browser.hidden = !blocked;
  if (p.needsAccount) {
    // An- und Abmelden sind Portalzugriffe: nie während eines Laufs, einer Pause oder bei erreichter Obergrenze.
    const why = locked() ? 'Erst nach dem laufenden Vorgang möglich'
      : paused ? 'Pausiert – bis dahin keine An- oder Abmeldung'
        : capped ? `Obergrenze erreicht – wieder ab ${formatTime(p.nextFreeAt)}`
          : null;
    const known = Boolean(p.sessionConfirmedAt) && !p.loginNeeded;
    c.login.disabled = Boolean(why);
    c.login.title = why ?? '';
    c.logout.disabled = Boolean(why) || !known;
    c.logout.title = why ?? (known ? '' : 'Keine bestätigte Anmeldung');
  }
  c.foot.hidden = !p.needsAccount && !blocked;
}

/** Wartet gerade ein Anmeldefenster? Gilt nur, solange der Lauf läuft. */
const waiting = (p) => state.busy && Boolean(state.run.loginWaiting[p.portal]);

function accountText(p) {
  if (waiting(p)) return 'Anmeldefenster ist offen';
  if (p.loginNeeded) return 'Anmeldung nötig';
  if (p.sessionConfirmedAt) return `angemeldet (zuletzt bestätigt ${formatTime(p.sessionConfirmedAt)})`;
  return 'unbekannt – wird beim nächsten Abruf geprüft';
}

/** An- oder Abmelden: sperrt die Oberfläche wie ein Lauf; „Abbrechen“ in der Laufleiste beendet es. */
/** Wurde der letzte Schritt von Hand abgebrochen? Dann ist „nicht bestätigt“ kein Fehler. */
let cancelled = false;

async function sessionStep(step, text) {
  setSession(text);
  cancelled = false;
  try {
    return await step();
  } finally {
    cancelled = state.cancelling;
    await refreshApp().catch(() => {});
    setSession(null);
    closeModal('quit');
  }
}

async function login() {
  let signedIn;
  try {
    signedIn = await sessionStep(api.portalLogin, 'Anmeldefenster für freelance.de ist offen – bitte dort anmelden …');
  } catch (error) {
    await errorBox('Anmeldung nicht möglich', error.message);
    return;
  }
  if (signedIn) {
    addLog('ok', 'Bei freelance.de angemeldet.');
    toast('Bei freelance.de angemeldet.', 'ok');
  } else {
    toast('Anmeldung nicht abgeschlossen – das Fenster wurde geschlossen, abgebrochen oder die Zeit lief ab.', 'warn');
  }
}

async function logout() {
  let done;
  try {
    done = await sessionStep(api.portalLogout, 'Abmeldung bei freelance.de …');
  } catch (error) {
    await errorBox('Abmeldung nicht möglich', error.message);
    return;
  }
  if (done) {
    addLog('info', 'Bei freelance.de abgemeldet.');
    toast('Bei freelance.de abgemeldet.', 'ok');
  } else if (cancelled) {
    toast('Abmeldung abgebrochen.', 'warn');
  } else {
    await errorBox('Abmeldung nicht bestätigt', 'Die Anmeldung besteht womöglich noch – der nächste Lauf ruft freelance.de dann weiter angemeldet ab.\n\n'
      + '„Alles zurücksetzen“ löscht die Anmeldung der App sicher.');
  }
}
