// Interaktions-Szenarien für den Prüfstand.
// Im Browser: `await (await import('/__checks.js')).run()`
// Jeder Fall beschreibt eine Zusage, die die Oberfläche einhalten muss.

const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const fake = window.__fake;
const calls = (cmd) => fake.calls.filter(([c]) => c === cmd).length;

const click = (node, detail = 1) =>
  node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail }));
const pointer = (node, type, button = 0) =>
  node.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, pointerId: 1, isPrimary: true, button }));
const key = (node, k) =>
  node.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: k }));

const rect = (node) => {
  const r = node.getBoundingClientRect();
  return `${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)}`;
};

const openSettings = async () => {
  if ($('#page-settings').hidden) click($$('#toolbar .btn.ghost.icon').at(-1));
  await wait(200);
};

const openJobs = async () => {
  if ($('#page-jobs').hidden) click($$('#toolbar .btn.ghost.icon').at(-1));
  await wait(200);
};

const cases = {
  /* ---------------------------------------------------------------- Eingabe */

  async 'Rechtsklick öffnet nirgends ein Kontextmenü'() {
    await openJobs();
    const targets = [$('.row'), $('#toolbar'), $('.reader'), $('#statusbar')].filter(Boolean);
    const blocked = targets.every((node) => {
      const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
      node.dispatchEvent(event);
      return event.defaultPrevented;
    });
    return { ok: blocked, detail: `${targets.length} Stellen geprüft` };
  },

  async 'Mittlere Maustaste löst nichts aus'() {
    await openJobs();
    const before = fake.calls.length;
    const down = new PointerEvent('pointerdown', { bubbles: true, cancelable: true, button: 1 });
    $('.row').dispatchEvent(down);
    const aux = new MouseEvent('auxclick', { bubbles: true, cancelable: true, button: 1 });
    $('.row').dispatchEvent(aux);
    await wait(100);
    return { ok: aux.defaultPrevented && fake.calls.length === before, detail: `Aufrufe +${fake.calls.length - before}` };
  },

  async 'Tasten wirken nur in Eingabefeldern'() {
    const outside = ['Enter', 'Escape', 'ArrowDown', 'F5', 'r'].map((k) => {
      const event = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: k });
      $('.row').dispatchEvent(event);
      return event.defaultPrevented;
    });
    await openSettings();
    const field = $('#gmail-user') ?? $('#search');
    const inside = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'a' });
    field.dispatchEvent(inside);
    return {
      ok: outside.every(Boolean) && !inside.defaultPrevented,
      detail: `außen gesperrt: ${outside.filter(Boolean).length}/5, im Feld frei: ${!inside.defaultPrevented}`,
    };
  },

  async 'Einfügen ins Passwortfeld bleibt möglich'() {
    await openSettings();
    const field = $('#gmail-password');
    if (!field) return { ok: false, detail: 'Passwortfeld fehlt' };
    const menu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    field.dispatchEvent(menu);
    const paste = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'v', ctrlKey: true });
    field.dispatchEvent(paste);
    return {
      ok: !menu.defaultPrevented && !paste.defaultPrevented,
      detail: `Kontextmenü frei: ${!menu.defaultPrevented}, Strg+V frei: ${!paste.defaultPrevented}`,
    };
  },

  async 'Ziehen von Text und Bildern ist aus'() {
    const event = new DragEvent('dragstart', { bubbles: true, cancelable: true });
    $('.reader').dispatchEvent(event);
    return { ok: event.defaultPrevented, detail: '' };
  },

  /* ---------------------------------------------------------------- Knöpfe */

  async 'Zehn Klicks auf „Abrufen“ starten einen Lauf'() {
    await openJobs();
    const before = calls('start_run');
    const run = $('#run');
    for (let i = 0; i < 10; i += 1) click(run);
    await wait(300);
    const busy = run.getAttribute('aria-busy') === 'true' || !$('#statusbar .spinner').hidden;
    await wait(2600);
    return { ok: calls('start_run') - before === 1, detail: `start_run ${calls('start_run') - before}, Kreisel ${busy}` };
  },

  async 'Ein Knopf behält nach dem Klick keinen Fokusring'() {
    const btn = $$('#toolbar .btn.ghost.icon')[0];
    btn.focus();
    click(btn);
    await wait(50);
    return { ok: document.activeElement !== btn, detail: `aktiv: ${document.activeElement.className || document.activeElement.tagName}` };
  },

  /* ---------------------------------------------------------------- Dialoge */

  async 'Im Dialog drücken, auf dem Hintergrund loslassen – er bleibt offen'() {
    await openSettings();
    const reset = $$('#page-settings .btn.danger').at(-1);
    click(reset);
    await wait(400);
    const sheet = $('#sheet');
    pointer($('.dialog-body'), 'pointerdown');
    click(sheet);
    await wait(80);
    const stillOpen = !sheet.hidden;
    click($('.dialog-foot .btn'));   // „Abbrechen“
    await wait(300);
    return { ok: stillOpen && sheet.hidden, detail: `offen nach Ziehen: ${stillOpen}, zu nach Klick: ${sheet.hidden}` };
  },

  async 'Der zweite Klick eines Doppelklicks trifft den Dialog nicht'() {
    await openSettings();
    const reset = $$('#page-settings .btn.danger').at(-1);
    click(reset);
    await wait(60);
    const sheet = $('#sheet');
    pointer(sheet, 'pointerdown');
    click(sheet, 2);
    await wait(60);
    const stillOpen = !sheet.hidden;
    click($('.dialog-foot .btn'));
    await wait(300);
    return { ok: stillOpen, detail: `offen nach Frühklick: ${stillOpen}` };
  },

  async 'Ein Dialog sperrt keine Taste – er hat nur Knöpfe'() {
    await openSettings();
    click($$('#page-settings .btn.danger').at(-1));
    await wait(400);
    const escape = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'Escape' });
    document.dispatchEvent(escape);
    await wait(80);
    const stillOpen = !$('#sheet').hidden;
    click($('.dialog-foot .btn'));
    await wait(300);
    return { ok: stillOpen && escape.defaultPrevented, detail: `Escape schließt nicht: ${stillOpen}` };
  },

  /* ---------------------------------------------------------------- Liste */

  async 'Ein Klick wählt die Zeile und öffnet den Text'() {
    await openJobs();
    const row = $$('.row')[2];
    click(row);
    await wait(400);
    return {
      ok: row.getAttribute('aria-selected') === 'true' && Boolean($('.reader-title')),
      detail: `Titel: ${$('.reader-title')?.textContent.slice(0, 24)}`,
    };
  },

  async 'Zeilenwechsel bei trägem Backend zeigt nie den vorigen Job'() {
    await openJobs();
    const rows = $$('.row');
    click(rows[1]);
    await wait(30);
    click(rows[4]);
    await wait(60);
    const titleNow = $('.reader-title')?.textContent ?? '';
    const wanted = rows[4].querySelector('.row-title').textContent;
    const loading = Boolean($('.reader-skeleton')) || Boolean($('.reader-text'));
    await wait(900);
    return {
      ok: titleNow === wanted,
      detail: `sofort richtig: ${titleNow === wanted}, Ladezustand: ${loading}`,
    };
  },

  async 'Zweihundert Fortschrittsereignisse rühren die Liste nicht an'() {
    await openJobs();
    const list = $('#list');
    let mutations = 0;
    const observer = new MutationObserver((records) => { mutations += records.length; });
    observer.observe(list, { childList: true, subtree: true, characterData: true });
    const channel = fake.calls.filter(([c]) => c === 'app_state').at(-1)?.[1]?.channel;
    for (let i = 0; i < 200; i += 1) channel?.onmessage?.({ type: 'progress', step: 'fetch', done: i, total: 200 });
    await wait(300);
    observer.disconnect();
    return { ok: mutations === 0, detail: `Mutationen ${mutations}` };
  },

  async 'Der Lauf verschiebt nichts im Aufbau'() {
    await openJobs();
    const before = [rect($('#run')), rect($('#list')), rect($('#statusbar'))].join(' | ');
    click($('#run'));
    await wait(700);
    const during = [rect($('#run')), rect($('#list')), rect($('#statusbar'))].join(' | ');
    await wait(2600);
    const after = [rect($('#run')), rect($('#list')), rect($('#statusbar'))].join(' | ');
    return { ok: before === during && during === after, detail: before === after ? 'gleich' : `${before} → ${after}` };
  },

  /* ---------------------------------------------------------------- Aufbau */

  async 'Liste und Lesebereich stehen nebeneinander, mit Umbruchregel'() {
    await openJobs();
    const list = $('#list').getBoundingClientRect();
    const reader = $('#reader').getBoundingClientRect();
    const nebeneinander = list.width > 200 && reader.left >= list.right - 1 && reader.width > list.width;
    // Das Fenster lässt sich im Prüfstand nicht verkleinern – die Regel wird gelesen.
    const rule = [...document.styleSheets]
      .flatMap((sheet) => { try { return [...sheet.cssRules]; } catch { return []; } })
      .some((r) => r.conditionText?.includes('859px'));
    return {
      ok: nebeneinander && rule,
      detail: `Liste ${Math.round(list.width)} px, Lesebereich ${Math.round(reader.width)} px, Umbruchregel ${rule}`,
    };
  },

  async 'Höchstens ein gefüllter Hauptknopf je Bildschirm'() {
    await openJobs();
    const row = $$('#list .row')[0];
    if (row) { click(row); await wait(300); }
    // Sichtbar heißt sichtbar – die Werkzeugleiste steht außerhalb der Seiten.
    const filled = () => $$('#app .btn.primary').filter((b) => b.offsetParent !== null);
    const jobs = filled();
    await openSettings();
    // „Ändern“ blendet das Postfach-Formular ein – erst dann steht „Speichern“ da.
    const change = $$('#page-settings .btn').find((b) => b.textContent.trim() === 'Ändern');
    if (change) { click(change); await wait(200); }
    const settings = filled();
    await openJobs();
    const label = (list) => list.map((b) => b.textContent.trim()).join(', ') || '–';
    return {
      ok: jobs.length <= 1 && settings.length <= 1,
      detail: `Jobs: ${label(jobs)} · Einstellungen: ${label(settings)}`,
    };
  },

  async 'Die Statusleiste hat immer genau einen Satz'() {
    const text = $('#status-text');
    const ok = text.textContent.trim().length > 0 && !text.textContent.includes('undefined');
    return { ok, detail: `„${text.textContent.slice(0, 48)}“` };
  },

  async 'Die Aktivität fährt auf und wieder zu'() {
    const panel = $('#panel');
    const toggle = $$('#statusbar .btn.ghost.icon').at(-1);
    click(toggle);
    await wait(350);
    const open = !panel.hidden && panel.classList.contains('is-open');
    click(toggle);
    await wait(350);
    return { ok: open && panel.hidden, detail: `auf: ${open}, zu: ${panel.hidden}` };
  },

  async 'Der Zeigerzustand löst sich erst bei echter Bewegung'() {
    dispatchEvent(new Event('blur'));
    const away = document.documentElement.classList.contains('pointer-away');
    dispatchEvent(new Event('focus'));
    const stillAway = document.documentElement.classList.contains('pointer-away');
    document.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, pointerId: 1 }));
    await wait(30);
    const back = !document.documentElement.classList.contains('pointer-away');
    return { ok: away && stillAway && back, detail: `weg ${away}, bleibt ${stillAway}, zurück ${back}` };
  },

  async 'Die Suche fragt erst, wenn man aufhört zu tippen'() {
    await openJobs();
    const before = calls('list_jobs');
    const field = $('#search');
    for (const value of ['S', 'SA', 'SAP']) {
      field.value = value;
      field.dispatchEvent(new Event('input', { bubbles: true }));
      await wait(40);
    }
    await wait(500);
    const after = calls('list_jobs') - before;
    field.value = '';
    field.dispatchEvent(new Event('input', { bubbles: true }));
    await wait(400);
    return { ok: after === 1, detail: `list_jobs ${after}` };
  },
};

export async function run() {
  const lines = [];
  for (const [name, fn] of Object.entries(cases)) {
    let result;
    try {
      result = await fn();
    } catch (error) {
      result = { ok: false, detail: `Ausnahme: ${error.message}` };
    }
    lines.push(`${result.ok ? 'OK  ' : 'FEHL'} ${name}${result.detail ? ` — ${result.detail}` : ''}`);
  }
  return lines.join('\n');
}

/** Leistung bei vielen Zeilen: `?jobs=5000` laden, dann `perf()`. */
export async function perf() {
  const rows = $$('.row');
  const start = performance.now();
  click(rows[Math.floor(rows.length / 2)]);
  await wait(0);
  const click_ms = performance.now() - start;
  return `Zeilen ${rows.length} · Klick ${click_ms.toFixed(1)} ms`;
}
