// Interaktions-Szenarien für den Prüfstand: im Browser `await (await import('/__checks.js')).run()`.
// Jeder Fall setzt die Seite neu auf (location.reload ist nicht nötig – Zustand wird zurückgesetzt,
// wo ein Fall ihn ändert) und liefert { name, ok, detail }.

const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const fake = window.__fake;
const calls = (cmd) => fake.calls.filter(([c]) => c === cmd).length;
const click = (node, detail = 1) => node.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail }));
const pointer = (node, type) => node.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, pointerId: 1, isPrimary: true }));
const view = async (name) => {
  click($(`[data-view=${name}]`));
  await wait(50);
};
const rect = (node) => {
  const r = node.getBoundingClientRect();
  return `${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)}`;
};

const cases = {
  async 'Dialog: im Dialog drücken, auf dem Hintergrund loslassen → bleibt offen'() {
    await view('system');
    click($('.system-grid .card:last-child .btn.danger'));
    await wait(350);
    const root = $('#modals');
    pointer($('.modal-body'), 'pointerdown');
    click(root);
    await wait(50);
    const open = !root.hidden && !root.classList.contains('is-leaving');
    pointer(root, 'pointerdown');
    click(root);
    await wait(300);
    return [open && root.hidden, `offen nach Drag: ${open}, zu nach echtem Klick: ${root.hidden}`];
  },
  async 'Dialog: zweiter Klick eines Doppelklicks (< 300 ms) wird ignoriert'() {
    click($('.system-grid .card:last-child .btn.danger'));
    await wait(30);
    const cancel = $$('.modal-foot .btn').find((b) => !b.classList.contains('danger'));
    click(cancel);
    await wait(30);
    const stillOpen = !$('#modals').hidden && !$('#modals').classList.contains('is-leaving');
    await wait(300);
    click(cancel);
    await wait(300);
    return [stillOpen && $('#modals').hidden && calls('reset_all') === 0, `offen nach Frühklick: ${stillOpen}`];
  },
  async 'Dialog: nach der Antwort bleibt die App 200 ms gesperrt'() {
    click($('.system-grid .card:last-child .btn.danger'));
    await wait(350);
    click($$('.modal-foot .btn').find((b) => !b.classList.contains('danger')));
    const inert = $('.app').inert;
    await wait(260);
    return [inert && !$('.app').inert && !$('#titlebar').inert, `inert direkt danach: ${inert}`];
  },
  async 'Dialog: 50× auf/zu – keydown-Listener bleiben ausgeglichen'() {
    let balance = 0;
    const add = document.addEventListener;
    const remove = document.removeEventListener;
    document.addEventListener = function (type, ...rest) { if (type === 'keydown') balance += 1; return add.call(this, type, ...rest); };
    document.removeEventListener = function (type, ...rest) { if (type === 'keydown') balance -= 1; return remove.call(this, type, ...rest); };
    try {
      for (let i = 0; i < 50; i += 1) {
        click($('.system-grid .card:last-child .btn.danger'));
        await wait(320);
        document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
        await wait(5);
      }
      await wait(250);
    } finally {
      document.addEventListener = add;
      document.removeEventListener = remove;
    }
    return [balance === 0 && $('#modals').hidden, `Bilanz ${balance}`];
  },
  async 'Knopf: 10 Klicks bei 300 ms Backend-Verzögerung → 1 Aufruf, Kreisel, danach frei'() {
    await view('profile');
    fake.delays.pick_profile = 300;
    const before = calls('pick_profile');
    const button = $('#view-profile .btn.primary');
    for (let i = 0; i < 10; i += 1) click(button);
    await wait(200);
    const busy = button.getAttribute('aria-busy') === 'true';
    await wait(300);
    delete fake.delays.pick_profile;
    return [calls('pick_profile') - before === 1 && busy && !button.hasAttribute('aria-busy'), `Aufrufe ${calls('pick_profile') - before}, Kreisel ${busy}`];
  },
  async 'Taste gehalten: wiederholtes Enter wird verworfen'() {
    const event = new KeyboardEvent('keydown', { key: 'Enter', repeat: true, bubbles: true, cancelable: true });
    $('#view-profile .btn.primary').dispatchEvent(event);
    return [event.defaultPrevented, `verworfen: ${event.defaultPrevented}`];
  },
  async 'Zeigerzustand: blur setzt pointer-away, erst Bewegung löst es'() {
    window.dispatchEvent(new Event('blur'));
    const set = document.documentElement.classList.contains('pointer-away');
    window.dispatchEvent(new Event('focus'));
    const still = document.documentElement.classList.contains('pointer-away');
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1 }));
    const cleared = !document.documentElement.classList.contains('pointer-away');
    return [set && still && cleared, `gesetzt ${set}, nach focus ${still}, nach Bewegung frei ${cleared}`];
  },
  async 'Fensterknöpfe: Minimieren setzt pointer-away, Titelleiste nie inert'() {
    click($('#win-min'));
    await wait(20);
    const away = document.documentElement.classList.contains('pointer-away');
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1 }));
    return [away && calls('window.minimize') >= 1 && !$('#titlebar').inert, `pointer-away ${away}`];
  },
  async 'Maximieren: 20 Größenänderungen → 1 isMaximized'() {
    await wait(150);
    const before = calls('window.isMaximized');
    for (let i = 0; i < 20; i += 1) window.dispatchEvent(new Event('resize'));
    await wait(200);
    return [calls('window.isMaximized') - before === 1, `Aufrufe ${calls('window.isMaximized') - before}`];
  },
  async 'Tabelle: Doppelklick auf Zeile → 1 job_detail, 1 open_target'() {
    await view('alerts');
    const detail = calls('job_detail');
    const open = calls('open_target');
    const tr = $$('#view-alerts tbody tr[data-key]')[3];
    click(tr);
    click(tr, 2);
    tr.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, detail: 2 }));
    tr.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, detail: 2 }));
    await wait(100);
    return [calls('job_detail') - detail === 1 && calls('open_target') - open === 1,
      `job_detail ${calls('job_detail') - detail}, open_target ${calls('open_target') - open}`];
  },
  async 'Tabelle: 30× Pfeil runter → höchstens 2 job_detail, Detail zeigt die letzte Zeile'() {
    const before = calls('job_detail');
    const wrap = $('.table-wrap');
    for (let i = 0; i < 30; i += 1) wrap.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, repeat: i > 0 }));
    await wait(400);
    const selected = $('tr.is-selected')?.dataset.key;
    const title = $('.detail h2').textContent;
    const job = fake.jobs.find((j) => `${j.key.portal}:${j.key.id}` === selected);
    return [calls('job_detail') - before <= 2 && job && title === job.title, `Aufrufe ${calls('job_detail') - before}`];
  },
  async 'Tabelle: Zeilenwechsel mit langsamem Backend zeigt nie den vorigen Job'() {
    fake.delays.job_detail = 400;
    const rows = $$('#view-alerts tbody tr[data-key]');
    click(rows[5]);
    await wait(20);
    const job = fake.jobs.find((j) => `${j.key.portal}:${j.key.id}` === rows[5].dataset.key);
    const title = $('.detail h2').textContent;
    const loading = !$('.detail .state').hidden;
    await wait(450);
    delete fake.delays.job_detail;
    return [title === job.title && loading, `Titel sofort richtig ${title === job.title}, Ladezustand ${loading}`];
  },
  async 'Lauf: 200 Fortschrittsereignisse ändern die Tabelle nicht'() {
    click($('.detail .btn.icon-only'));
    click($('#topbar-actions .btn.primary'));
    await wait(400);
    let mutations = 0;
    const observer = new MutationObserver((list) => { mutations += list.length; });
    observer.observe($('#view-alerts tbody'), { childList: true, subtree: true, characterData: true, attributes: true });
    for (let i = 0; i < 200; i += 1) fake.emit({ type: 'progress', step: 'fetch', done: i, total: 200 });
    await wait(50);
    observer.disconnect();
    return [mutations === 0, `Mutationen ${mutations}`];
  },
  async 'Lauf: Laufleiste und Abbrechen in jeder Ansicht'() {
    const seen = [];
    for (const name of ['portals', 'profile', 'system', 'alerts']) {
      await view(name);
      seen.push(!$('#run-cancel').hidden && $('#runbar').getBoundingClientRect().height > 0);
    }
    return [seen.every(Boolean), seen.join(',')];
  },
  async 'Lauf: Start-Klick während des Abschluss-Neuladens startet nichts'() {
    fake.delays.app_state = 400;
    fake.finish();
    await wait(50);
    const before = calls('start_run');
    click($('#topbar-actions .btn.primary'));
    await wait(500);
    delete fake.delays.app_state;
    await wait(400);
    return [calls('start_run') === before && $('#modals').hidden === false, `start_run ${calls('start_run') - before}`];
  },
  async 'Abschlussdialog schließen, keine Layout-Verschiebung der Kopfknöpfe'() {
    const ok = $$('.modal-foot .btn').find((b) => b.classList.contains('primary'));
    await wait(350);
    click(ok);
    await wait(300);
    const button = $('#topbar-actions .btn.primary');
    const before = rect(button);
    const runbar = rect($('#runbar'));
    click(button);
    await wait(300);
    const during = rect(button);
    const runbarDuring = rect($('#runbar'));
    fake.finish();
    await wait(700);
    click($$('.modal-foot .btn').find((b) => b.classList.contains('primary')));
    await wait(700);
    return [before === during && runbar === runbarDuring, `${before} → ${during}; Laufleiste ${runbar} → ${runbarDuring}`];
  },
  async 'Gmail tippen verschiebt keine Knöpfe'() {
    await view('system');
    const buttons = $$('#view-system .card:first-child .card-foot .btn');
    const before = buttons.map(rect).join('|');
    const input = $('#gmail-password');
    input.value = 'abcd';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await wait(30);
    const after = buttons.map(rect).join('|');
    input.value = '';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    return [before === after, before === after ? 'gleich' : `${before} → ${after}`];
  },
};

export async function run(only = null) {
  const results = [];
  for (const [name, fn] of Object.entries(cases)) {
    if (only && !name.includes(only)) continue;
    try {
      const [ok, detail] = await fn();
      results.push({ ok, name, detail });
    } catch (error) {
      results.push({ ok: false, name, detail: String(error?.stack || error) });
    }
  }
  window.__results = results;
  return results.map((r) => `${r.ok ? 'OK  ' : 'FAIL'} ${r.name} — ${r.detail}`).join('\n');
}

export async function perf() {
  // 5000 Zeilen: eine Einstellung umschalten und eine Pfeiltaste – in ms.
  const t0 = performance.now();
  click($$('.settings input[name=format]')[1]);
  const settings = performance.now() - t0;
  const wrap = $('.table-wrap');
  const t1 = performance.now();
  wrap.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
  const arrow = performance.now() - t1;
  return { rows: $$('#view-alerts tbody tr').length, settings: Math.round(settings), arrow: Math.round(arrow) };
}
