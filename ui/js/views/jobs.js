// Ergebnisse der Ansicht „Job-Alerts“: Reiter, Suche, Tabelle, Auswahl per Maus und Tastatur,
// Jobdetails. Zeilen werden nur angelegt, wenn sie neu sind, und sonst an Ort und Stelle
// aktualisiert – nie unter dem Zeiger ersetzt.

import { api } from '../api.js';
import { openTarget } from '../actions.js';
import { portalLabel, startRun } from '../run.js';
import { locked, state, subscribe } from '../store.js';
import { el, errorBox, formatShort, formatTime, icon, plural, toast } from '../ui.js';

const COLUMNS = [['source', 'Portal'], ['date', 'Datum'], ['job', 'Job · Unternehmen · Ort'], ['details', 'Details']];

const keyOf = (key) => `${key.portal}:${key.id}`;

const jobs = {
  tab: 'new',
  search: '',
  /** Schlüssel → JobView, in Anzeigereihenfolge. */
  rows: new Map(),
  newCount: null,
  /** Gewählter Job { id, key } – bleibt, auch wenn er aus der Liste fällt. */
  selected: null,
  /** Laufende Nummern: nur die neueste Antwort gilt. */
  listSeq: 0,
  detailSeq: 0,
};
/** Gebaute Zeilen: Job-Schlüssel → { sig, tr, cells }, Mail-Schlüssel → tr. */
const built = new Map();
const mailRows = new Map();
const n = {};

/** Baut Ergebniskarte und Detailkarte; `side` ist die Spalte, in der die Details erscheinen. */
export function build(side) {
  n.side = side;
  const results = resultsCard();
  const detail = detailCard();
  subscribe('app', () => {
    renderTabs();
    renderTable();
    renderDetailActions();
  });
  subscribe('busy', () => {
    renderTable();
    renderDetailActions();
  });
  let runSig = '';
  subscribe('run', (run) => {
    const sig = `${Object.keys(run.stops).join()}|${run.emptyAlerts.length}`;
    if (sig === runSig) return;
    runSig = sig;
    renderTable();
  });
  subscribe('job', onJob);
  subscribe('jobs', onJobs);
  reload();
  return { results, detail };
}

/** Beim Start eines Postfach-Abrufs springt die Ansicht auf „Neu in diesem Lauf“. */
export function showNew() {
  setTab('new');
}

/* ------------------------------------------------------------------ Ergebniskarte */

function resultsCard() {
  const tab = (value, text) => el('button', {
    class: 'tab', type: 'button', 'aria-pressed': 'false', onclick: () => setTab(value),
  }, el('span', { text }), el('span', { class: 'tab-count', text: '–' }));
  n.tabNew = tab('new', 'Neu in diesem Lauf');
  n.tabAll = tab('all', 'Alle');
  let timer = 0;
  n.search = el('input', {
    type: 'text', class: 'search', placeholder: 'Suchen, auch in den Jobdetails',
    spellcheck: 'false', autocomplete: 'off', 'aria-label': 'Jobs durchsuchen',
    oninput: () => {
      clearTimeout(timer);
      timer = setTimeout(() => setSearch(n.search.value), 250);
    },
    onkeydown: (event) => {
      if (event.key === 'Enter') {
        clearTimeout(timer);
        setSearch(n.search.value);
      } else if (event.key === 'Escape' && n.search.value) {
        event.stopPropagation();
        n.search.value = '';
        clearTimeout(timer);
        setSearch('');
      }
    },
  });
  n.tbody = el('tbody', { onclick: onRowClick, ondblclick: onRowDoubleClick });
  n.emptyTitle = el('span', { class: 'state-title' });
  n.emptyText = el('span');
  n.emptySpinner = el('span', { class: 'spinner' });
  n.emptyRow = el('tr', null, el('td', { class: 'state-cell', colspan: String(COLUMNS.length) },
    el('div', { class: 'state' }, n.emptySpinner, n.emptyTitle, n.emptyText)));
  n.wrap = el('div', { class: 'table-wrap', tabindex: '0', 'aria-label': 'Gefundene Jobs', onkeydown: onTableKey },
    el('table', { class: 'table' },
      el('thead', null, el('tr', null, COLUMNS.map(([col, text]) => el('th', { dataset: { col }, text })))),
      n.tbody));
  n.info = el('span');
  n.results = el('div', { class: 'card results' },
    el('div', { class: 'card-head' },
      el('div', { class: 'card-title' },
        el('h2', { text: 'Gefundene Jobs' }),
        el('p', { text: 'Doppelklick öffnet die Anzeige im Browser' })),
      el('div', { class: 'card-tools' }, el('div', { class: 'tabs', role: 'group', 'aria-label': 'Anzeige' }, n.tabNew, n.tabAll), n.search)),
    el('div', { class: 'card-body flush' }, n.wrap),
    el('div', { class: 'card-foot' }, n.info));
  return n.results;
}

function setTab(tab) {
  if (jobs.tab === tab) return;
  jobs.tab = tab;
  renderTabs();
  reload();
}

function setSearch(text) {
  const search = text.trim();
  if (search === jobs.search) return;
  jobs.search = search;
  reload();
}

function renderTabs() {
  n.tabNew.lastChild.textContent = jobs.newCount === null ? '–' : String(jobs.newCount);
  n.tabAll.lastChild.textContent = String(state.app.jobsTotal);
  n.tabNew.setAttribute('aria-pressed', String(jobs.tab === 'new'));
  n.tabAll.setAttribute('aria-pressed', String(jobs.tab === 'all'));
}

function onJobs(reason) {
  if (reason === 'start') {
    if (state.run.kind !== 'scan' || jobs.tab !== 'new') return;
    // Neuer Postfach-Abruf: „Neu in diesem Lauf“ beginnt leer; eine noch laufende Abfrage gilt nicht mehr.
    jobs.listSeq += 1;
    jobs.rows = new Map();
    if (!jobs.search) jobs.newCount = 0;
    renderTabs();
    renderTable();
    reloadSoon();
    return;
  }
  // Neue Alert-Mails betreffen nur den Reiter „Neu“.
  if (reason === 'alert' && jobs.tab !== 'new') return;
  reloadSoon();
}

let reloadTimer = 0;

/** Nachladen höchstens alle 300 ms (Alert-Ereignisse kommen dicht). */
function reloadSoon() {
  if (reloadTimer) return;
  reloadTimer = setTimeout(() => {
    reloadTimer = 0;
    reload();
  }, 300);
}

async function reload() {
  const seq = ++jobs.listSeq;
  // Ladelinie am Kartenkopf (erscheint erst nach 150 ms – kurze Abfragen bleiben ruhig).
  n.results.setAttribute('aria-busy', 'true');
  let list;
  try {
    list = await api.listJobs({ latestRun: jobs.tab === 'new', search: jobs.search || null });
  } catch (error) {
    if (seq === jobs.listSeq) toast(`Die Jobliste ließ sich nicht laden: ${error.message}`, 'error');
    return;
  } finally {
    if (seq === jobs.listSeq) n.results.removeAttribute('aria-busy');
  }
  if (seq !== jobs.listSeq) return;
  jobs.rows = new Map(list.map((job) => [keyOf(job.key), job]));
  if (!jobs.search) {
    jobs.newCount = jobs.tab === 'new' ? list.length : list.filter((j) => j.firstSeenRun === state.app.lastScanRun).length;
  }
  renderTabs();
  renderTable();
}

function onJob(job) {
  const key = keyOf(job.key);
  if (jobs.rows.has(key)) {
    jobs.rows.set(key, job);
    jobRow(key, job);
    renderInfo();
  }
  if (jobs.selected?.id === key) {
    renderDetailShell(job);
    loadDetail();
  }
}

/* ------------------------------------------------------------------ Tabelle */

/** Alert-Mails ohne Einträge: während eines Postfach-Abrufs aus den Ereignissen, sonst gespeichert. */
function emptyMails() {
  if (state.busy && state.run.kind === 'scan') {
    return state.run.emptyAlerts.map((a) => ({
      portalLabel: portalLabel(a.portal), subject: a.subject, mailDate: a.date, gmailId: a.gmailId,
    }));
  }
  return state.app.zeroPostingMails;
}

function renderTable() {
  const rows = [];
  const mails = new Set();
  if (jobs.tab === 'new' && !jobs.search) {
    for (const mail of emptyMails()) {
      const key = mail.gmailId || `${mail.subject}|${mail.mailDate}`;
      mails.add(key);
      if (!mailRows.has(key)) mailRows.set(key, mailRow(mail));
      rows.push(mailRows.get(key));
    }
  }
  for (const key of mailRows.keys()) if (!mails.has(key)) mailRows.delete(key);
  for (const [key, job] of jobs.rows) rows.push(jobRow(key, job));
  for (const key of built.keys()) if (!jobs.rows.has(key)) built.delete(key);
  if (!rows.length) {
    renderEmpty();
    rows.push(n.emptyRow);
  }
  // Nur Geändertes anfassen: Unveränderte Zeilen bleiben stehen – sonst rechnet der
  // Browser bei tausenden Zeilen jedes Mal die ganze Tabelle neu.
  const keep = new Set(rows);
  for (const tr of [...n.tbody.children]) if (!keep.has(tr)) tr.remove();
  rows.forEach((tr, i) => {
    const at = n.tbody.children[i];
    if (at !== tr) n.tbody.insertBefore(tr, at ?? null);
  });
  renderInfo();
}

function renderEmpty() {
  const scanning = state.busy && state.run.kind === 'scan';
  let title;
  let text;
  if (jobs.search) [title, text] = ['Keine Treffer', `Für „${jobs.search}“ wurde nichts gefunden.`];
  else if (jobs.tab === 'all') [title, text] = ['Noch keine Jobs gespeichert', '„Postfach abrufen“ startet die Suche.'];
  else if (scanning) [title, text] = ['Noch keine neuen Jobs in diesem Lauf', ''];
  else if (!state.app.lastScanRun) [title, text] = ['Noch keine Jobs', '„Postfach abrufen“ startet die Suche.'];
  else [title, text] = ['Keine neuen Jobs im letzten Lauf', '„Alle“ zeigt alle bisher gefundenen.'];
  n.emptyTitle.textContent = title;
  n.emptyText.textContent = text;
  n.emptySpinner.hidden = !scanning;
}

function mailRow(mail) {
  return el('tr', {
    class: 'is-mail',
    title: mail.gmailId ? 'Doppelklick öffnet die Mail in Gmail' : null,
    dataset: mail.gmailId ? { gmail: mail.gmailId } : null,
  },
  el('td', { dataset: { col: 'source' }, text: mail.portalLabel }),
  el('td', { dataset: { col: 'date' }, title: formatTime(mail.mailDate), text: formatShort(mail.mailDate) }),
  el('td', { dataset: { col: 'job' }, title: mail.subject },
    el('span', { class: 'job-title', text: `Alert-Mail: ${mail.subject}` }),
    el('span', { class: 'job-meta', text: 'keine Jobs erkannt' })),
  el('td', { dataset: { col: 'details' } }));
}

/** Zeile eines Jobs – neu angelegt oder an Ort und Stelle aktualisiert. */
function jobRow(key, job) {
  // „übersprungen“ nur während des Laufs – danach gilt wieder „fehlt“.
  const stop = state.busy && job.status === 'missing' ? state.run.stops[job.key.portal] : null;
  const sig = [job.portalLabel, job.mailDate, job.firstSeenAt, job.title, job.company, job.location, job.status,
    job.short, job.closed, job.descLen, job.txtName, job.descError, job.statusText, job.mailSubject, job.gmailUrl,
    stop?.text].join('');
  let entry = built.get(key);
  if (!entry) {
    const cells = Object.fromEntries(COLUMNS.map(([col]) => [col, el('td', { dataset: { col } })]));
    entry = { sig: '', tr: el('tr', { dataset: { key } }, Object.values(cells)), cells };
    built.set(key, entry);
  }
  if (entry.sig !== sig) {
    entry.sig = sig;
    fillRow(entry.cells, job, stop);
  }
  entry.tr.classList.toggle('is-selected', jobs.selected?.id === key);
  return entry.tr;
}

function fillRow(cells, job, stop) {
  cells.source.textContent = job.portalLabel;
  cells.date.textContent = formatShort(job.mailDate ?? job.firstSeenAt);
  cells.date.title = `${formatTime(job.mailDate ?? job.firstSeenAt)} · Mail: ${job.mailSubject}${job.gmailUrl ? ' – Doppelklick öffnet die Mail in Gmail' : ''}`;
  const meta = [job.company, job.location].filter(Boolean).join(' · ');
  cells.job.replaceChildren(el('span', { class: 'job-title', text: job.title }), el('span', { class: 'job-meta', text: meta || '–' }));
  cells.job.title = meta ? `${job.title}
${meta}` : job.title;
  const [text, level, title] = badge(job, stop);
  // Abzeichen tragen nie ein Symbol – die Stufe steckt in der Farbe, der Zustand im Wort.
  cells.details.replaceChildren(el('span', { class: `badge ${level}`.trim(), title, text }));
}

/** Abzeichen „Details“: [Text, Stufe, Tooltip] – jeder Job hat einen echten Zustand. */
function badge(job, stop) {
  switch (job.status) {
    case 'ok':
      if (job.closed) return ['geschlossen', '', 'Jobdetails vorhanden.'];
      if (job.short) return ['kurz', 'ok', `${job.statusText} (${job.descLen} Zeichen)`];
      return ['vorhanden', 'ok', `${job.descLen} Zeichen · Textdatei: ${job.txtName ?? 'folgt beim nächsten Export'}`];
    case 'missing':
      return stop
        ? ['übersprungen', 'warn', stop.text]
        : ['fehlt', 'warn', '„Jobdetails extrahieren“ lädt sie nach.'];
    case 'failed':
      return ['fehlt', 'warn', `Letzter Fehler: ${job.descError ?? job.statusText}`];
    case 'gone':
      return ['abgelaufen', '', null];
    default:
      return ['nicht abrufbar', '', `Nach mehreren Fehlversuchen aufgegeben: ${job.descError ?? job.statusText}`];
  }
}

function renderInfo() {
  const total = jobs.rows.size;
  let done = 0;
  for (const job of jobs.rows.values()) if (job.status === 'ok') done += 1;
  const count = plural(total, 'Job', 'Jobs');
  n.info.textContent = !total ? ''
    : !done ? `${count} · noch keine Jobdetails`
      : done < total ? `${count} · ${done} mit Jobdetails`
        : `${count} · alle mit Jobdetails`;
}

function onRowClick(event) {
  const tr = event.target.closest('tr[data-key]');
  if (tr) select(tr.dataset.key);
}

function onRowDoubleClick(event) {
  const tr = event.target.closest('tr');
  if (tr?.dataset.gmail) {
    openTarget({ kind: 'gmail', id: tr.dataset.gmail });
    return;
  }
  const job = tr && jobs.rows.get(tr.dataset.key);
  if (!job) return;
  const mail = event.target.closest('td')?.dataset.col === 'date' && job.gmailUrl;
  openTarget(mail ? { kind: 'mailUrl', key: job.key } : { kind: 'jobUrl', key: job.key });
}

function onTableKey(event) {
  if (event.key === 'Escape' && jobs.selected) {
    event.preventDefault();
    select(null);
    return;
  }
  const keys = [...jobs.rows.keys()];
  if (!keys.length) return;
  const at = keys.indexOf(jobs.selected?.id);
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    const next = event.key === 'ArrowDown' ? Math.min(keys.length - 1, at + 1) : Math.max(0, at < 0 ? 0 : at - 1);
    // Beim Halten der Taste folgt die Auswahl sofort, geladen wird erst, wenn sie steht.
    select(keys[next], { debounce: true });
    built.get(keys[next])?.tr.scrollIntoView({ block: 'nearest' });
  } else if (event.key === 'Enter' && jobs.selected) {
    event.preventDefault();
    openTarget({ kind: 'jobUrl', key: jobs.selected.key });
  }
}

/* ------------------------------------------------------------------ Jobdetails */

function detailCard() {
  n.detailTitle = el('h2');
  n.detailMeta = el('p');
  n.detailKv = el('dl', { class: 'kv' });
  n.detailText = el('div', { class: 'detail-text' });
  n.detailLoading = el('div', { class: 'state' }, el('span', { class: 'spinner' }), 'Jobdetails werden geladen …');
  n.openJob = el('button', { class: 'btn primary', type: 'button', text: 'Anzeige öffnen', onclick: () => jobs.selected && openTarget({ kind: 'jobUrl', key: jobs.selected.key }) });
  n.openMail = el('button', { class: 'btn', type: 'button', text: 'Mail öffnen', onclick: () => jobs.selected && openTarget({ kind: 'mailUrl', key: jobs.selected.key }) });
  n.fetchJob = el('button', { class: 'btn', type: 'button', text: 'Details holen', onclick: () => jobs.selected && startRun('job', [jobs.selected.key]) });
  n.detail = el('div', { class: 'card detail', hidden: true, role: 'region', 'aria-label': 'Jobdetails' },
    el('div', { class: 'card-head' },
      el('div', { class: 'card-title' }, n.detailTitle, n.detailMeta),
      el('button', { class: 'btn icon-only', type: 'button', title: 'Details schließen (Esc)', 'aria-label': 'Details schließen', onclick: () => select(null) }, icon('close'))),
    el('div', { class: 'card-body' }, n.detailKv, n.detailLoading, n.detailText),
    el('div', { class: 'card-foot' }, n.openJob, n.openMail, n.fetchJob));
  n.detail.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') select(null);
  });
  return n.detail;
}

let detailTimer = 0;

function select(id, { debounce = false } = {}) {
  if ((jobs.selected?.id ?? null) === (id ?? null)) return;
  const job = id ? jobs.rows.get(id) : null;
  if (jobs.selected) built.get(jobs.selected.id)?.tr.classList.remove('is-selected');
  jobs.selected = job ? { id, key: job.key } : null;
  clearTimeout(detailTimer);
  jobs.detailSeq += 1;
  if (!job) {
    n.side.classList.remove('has-detail');
    n.detail.hidden = true;
    return;
  }
  built.get(id)?.tr.classList.add('is-selected');
  // Kopf und Eckdaten sofort aus der Zeile – nie der vorige Job unter dem neuen Titel.
  renderDetailShell(job);
  n.detailText.hidden = true;
  n.detailLoading.hidden = false;
  n.side.classList.add('has-detail');
  n.detail.hidden = false;
  if (debounce) detailTimer = setTimeout(loadDetail, 150);
  else loadDetail();
}

async function loadDetail() {
  const selected = jobs.selected;
  if (!selected) return;
  const seq = ++jobs.detailSeq;
  let detail;
  try {
    detail = await api.jobDetail(selected.key);
  } catch (error) {
    if (seq !== jobs.detailSeq) return;
    if (error.kind === 'notFound') {
      select(null);
      toast(error.message, 'warn');
    } else {
      n.detailLoading.hidden = true;
      await errorBox('Jobdetails nicht geladen', error.message);
    }
    return;
  }
  if (seq !== jobs.detailSeq) return;
  renderDetailShell(detail.job);
  n.detailText.textContent = detail.text || EMPTY_TEXT[detail.job.status](detail.job);
  n.detailText.classList.toggle('muted', !detail.text);
  n.detailLoading.hidden = true;
  n.detailText.hidden = false;
}

const EMPTY_TEXT = {
  missing: () => 'Noch keine Jobdetails. „Details holen“ lädt sie.',
  failed: (job) => `Keine Jobdetails: ${job.descError ?? 'ohne Angabe'}. „Details holen“ versucht es erneut.`,
  gone: () => 'Die Anzeige gibt es nicht mehr.',
  unfetchable: () => 'Nach mehreren Fehlversuchen aufgegeben.',
  ok: () => '',
};

function renderDetailShell(job) {
  n.detailTitle.textContent = job.title;
  n.detailMeta.textContent = [job.portalLabel, job.company, job.location].filter(Boolean).join(' · ');
  const rows = [
    ['Mail', job.mailSubject || '–'],
    ['Mail-Datum', formatTime(job.mailDate) || '–'],
    ['Zuerst gesehen', formatTime(job.firstSeenAt)],
    ['Jobdetails', job.descLen ? `${job.statusText}, ${job.descLen} Zeichen` : job.statusText],
    ['Textdatei', job.txtName ?? 'noch nicht geschrieben'],
  ];
  n.detailKv.replaceChildren(...rows.flatMap(([k, v]) => [el('dt', { text: k }), el('dd', { text: v })]));
  n.detailJob = job;
  renderDetailActions();
}

function renderDetailActions() {
  const job = n.detailJob;
  if (!job) return;
  n.openMail.disabled = !job.gmailUrl;
  n.openMail.title = job.gmailUrl ? 'Die Alert-Mail in Gmail öffnen' : 'Zu diesem Job ist keine Gmail-Mail bekannt.';
  const portal = state.app.portals.find((p) => p.portal === job.key.portal);
  n.fetchJob.disabled = locked() || job.status === 'ok';
  n.fetchJob.title = job.status === 'ok' ? 'Jobdetails liegen bereits vor'
    : locked() ? 'Erst nach dem laufenden Vorgang möglich'
      : portal?.pausedUntil ? `${portal.label} pausiert bis ${formatTime(portal.pausedUntil)}`
        : '';
}
