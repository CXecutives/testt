// Typed stand-in for @tauri-apps/api in the harness build (`vite build --mode harness`
// aliases every `@tauri-apps/api/*` import to this file). It answers every IPC command
// (typed by the generated `Commands` map) from a small in-memory store with realistic,
// invented sample data, records the calls and lets a test push run events:
//
//   window.__harness.calls          [command, args][]
//   window.__harness.emit(event)    deliver a RunEvent on every open channel
//   window.__harness.done           true once a started run has finished
//
// Scenarios (`?scenario=`): default · first-run · mailbox-only · no-profile · empty ·
// many (2000 jobs) · offline · paused · running · slow · list-error · reset.
// `?tick=ms` sets the pace of a scripted run (default 40). Dates are fixed so screenshots
// stay stable (the tests also fix the clock).

import type {
  AppState,
  Commands,
  ErrorInfo,
  Highlight,
  JobCounts,
  JobDetail,
  JobQuery,
  JobView,
  PortalState,
  ProfileInfo,
  Reason,
  RunEvent,
  RunSummary,
} from '../../ui/src/lib/ipc/types';

interface Harness {
  calls: [string, unknown][];
  emit: (event: RunEvent) => void;
  done: boolean;
}

declare global {
  interface Window {
    __harness: Harness;
  }
}

/* ------------------------------------------------------------------ channel */

const channels = new Set<Channel<RunEvent>>();

export class Channel<T = unknown> {
  onmessage: (message: T) => void = () => undefined;
}

/* ----------------------------------------------------------------- scenario */

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'default';
const TICK = Number(params.get('tick') ?? 40);
const DELAY = scenario === 'slow' ? 900 : 0;

const NOW = new Date('2026-09-24T09:30:00+02:00').getTime();
const HOUR = 3_600_000;
const at = (hoursAgo: number): string => new Date(NOW - hoursAgo * HOUR).toISOString();
const later = (minutes: number): string => new Date(NOW + minutes * 60_000).toISOString();

/* ----------------------------------------------------------------- fixtures */

type Match = NonNullable<JobView['match']>;

const scored = (score: number, top: string[], mustMet = 3, mustTotal = 4): Match => ({
  score,
  band: score >= 80 ? 'high' : score >= 40 ? 'mid' : 'low',
  status: 'scored',
  note: null,
  mustMet,
  mustTotal,
  top,
});

const excludedBy = (criterion: string, score: number): Match => ({
  score,
  band: score >= 80 ? 'high' : score >= 40 ? 'mid' : 'low',
  status: 'excluded',
  note: { code: 'hardCriterion', params: { criterion } },
  mustMet: 2,
  mustTotal: 4,
  top: [],
});

function job(
  portal: JobView['portal'],
  id: string,
  title: string,
  company: string,
  location: string,
  hoursAgo: number,
  extra: Partial<JobView> = {},
): JobView {
  return {
    key: { portal, id },
    portal,
    title,
    company,
    location,
    workMode: 'hybrid',
    mailDate: at(hoursAgo),
    firstSeenAt: at(hoursAgo),
    unread: false,
    pinned: false,
    detail: { kind: 'ok' },
    short: false,
    match: null,
    alsoOn: [],
    ...extra,
  };
}

function sampleJobs(): JobView[] {
  return [
    job(
      'freelancermap',
      '2801',
      'Interim CFO (m/w/d) für Familienunternehmen',
      'Hanseatic Holding GmbH',
      'Hamburg',
      2,
      {
        unread: true,
        pinned: true,
        alsoOn: ['linkedin'],
        match: scored(91, ['Interim-Management im Mittelstand', 'Konzernabschluss nach HGB'], 4, 4),
      },
    ),
    job(
      'linkedin',
      '4100200301',
      'Head of Controlling Transformation',
      'Nordlicht Energie AG',
      'Bremen',
      3,
      {
        unread: true,
        workMode: 'remote',
        match: scored(84, ['Controlling mit SAP S/4HANA', 'Aufbau Reporting'], 4, 5),
      },
    ),
    job(
      'freelance',
      '900411',
      'SAP S/4HANA Finance Projektleitung',
      'Datenwerk Süd GmbH',
      'München',
      5,
      {
        unread: true,
        detail: { kind: 'teaser' },
        match: scored(76, ['Projektleitung SAP Finance'], 2, 3),
      },
    ),
    job('freelancermap', '2802', 'Interim Head of Finance', 'Grünwerk Mobility GmbH', 'Berlin', 6, {
      unread: true,
      workMode: 'remote',
      match: scored(72, ['Finanzplanung und Liquidität'], 3, 4),
    }),
    job(
      'freelancermap',
      '2803',
      'Kaufmännische Leitung Projektgeschäft',
      'Werft 7 GmbH',
      'Kiel',
      9,
      {
        unread: true,
        workMode: 'onsite',
        match: scored(66, ['Projektcontrolling'], 2, 4),
      },
    ),
    job(
      'linkedin',
      '4100200302',
      'Finance Business Partner Shared Service',
      'Alpenblick Logistik AG',
      'Leipzig',
      11,
      {
        unread: true,
        detail: { kind: 'pending', retryAt: null },
      },
    ),
    job(
      'freelance',
      '900412',
      'Buchhaltung über Personaldienstleister',
      'Musterpersonal GmbH',
      'Berlin',
      12,
      {
        unread: true,
        workMode: null,
        match: excludedBy('noAnue', 55),
      },
    ),
    job(
      'linkedin',
      '4100200303',
      'Controller Konzernberichtswesen',
      'Contoso Services GmbH',
      'Frankfurt am Main',
      27,
      {
        match: scored(58, ['Konzernberichtswesen'], 2, 4),
      },
    ),
    job('freelancermap', '2804', 'Interim Treasury Manager', 'Rheinhafen Chemie GmbH', 'Köln', 30, {
      match: scored(47, ['Liquiditätsplanung'], 1, 3),
    }),
    job(
      'linkedin',
      '4100200304',
      'Leitung Rechnungswesen',
      'Stadtwerke Nordheide',
      'Buchholz',
      50,
      {
        workMode: 'onsite',
        match: scored(45, ['Jahresabschluss nach HGB'], 2, 4),
      },
    ),
    job('freelance', '900413', 'SAP FI Berater Migration', 'Datenwerk Süd GmbH', 'München', 55, {
      workMode: 'onsite',
      match: scored(32, ['SAP FI'], 1, 4),
    }),
    job(
      'freelancermap',
      '2805',
      'Projektcontroller Bau',
      'Baufeld Projekte GmbH',
      'Stuttgart',
      70,
      {
        detail: { kind: 'failed', attempts: 3, retryAt: null },
        match: scored(24, [], 0, 3),
      },
    ),
    job('linkedin', '4100200305', 'Payroll Specialist', 'Lakeside Payroll AG', 'Zürich', 80, {
      match: excludedBy('countries', 38),
    }),
    job('freelancermap', '2806', 'Reporting Analyst', 'Hafenkontor GmbH', 'Hamburg', 96, {
      short: true,
      match: {
        score: 0,
        band: 'low',
        status: 'unscorable',
        note: { code: 'shortText', params: {} },
        mustMet: 0,
        mustTotal: 0,
        top: [],
      },
    }),
  ];
}

const COMPANIES = [
  'Nordlicht Energie AG',
  'Werft 7 GmbH',
  'Contoso Services GmbH',
  'Alpenblick Logistik AG',
];
const TITLES = [
  'Interim Controller',
  'SAP FI/CO Berater',
  'Finance Manager',
  'Projektleitung Finance',
];
const CITIES = ['Hamburg', 'Berlin', 'München', 'Köln', 'Leipzig'];

function manyJobs(count: number): JobView[] {
  const out: JobView[] = [];
  const portals = ['linkedin', 'freelancermap', 'freelance'] as const;
  for (let i = 0; i < count; i += 1) {
    const score = (i * 37) % 100;
    out.push(
      job(
        portals[i % 3]!,
        String(100000 + i),
        `${TITLES[i % TITLES.length]} ${i + 1}`,
        COMPANIES[i % COMPANIES.length]!,
        CITIES[i % CITIES.length]!,
        i / 4,
        {
          unread: i % 3 === 0,
          match:
            i % 17 === 5
              ? excludedBy('minDayRate', score)
              : scored(score, ['Controlling im Konzern']),
        },
      ),
    );
  }
  return out;
}

const PROFILE: ProfileInfo = {
  fileName: 'profil-interim-finance.json',
  bytes: 18_422,
  savedAt: at(72),
  quality: 'good',
  understood: {
    competenceCount: 42,
    competences: [
      'Interim-Management',
      'Konzernabschluss nach HGB',
      'Controlling',
      'SAP S/4HANA Finance',
      'Liquiditätsplanung',
      'Restrukturierung',
      'M&A Integration',
      'Reporting',
      'Treasury',
      'Budgetierung',
      'IFRS',
      'Führung von Finanzteams',
    ],
    sources: ['kernkompetenzen[].kompetenz', 'projekte[].rolle'],
    criteria: [
      { code: 'minDayRate', params: { set: true, rate: 1100 } },
      { code: 'countries', params: { set: true, countries: 'Deutschland, Österreich' } },
      { code: 'noAnue', params: { set: true } },
      { code: 'availability', params: { set: false } },
    ],
    warnings: [{ code: 'ignoredKeys', params: { keys: 'hobbys, referenzen' } }],
  },
  scoredAt: at(1),
  pending: 0,
  parseError: null,
};

const portal = (name: PortalState['portal'], extra: Partial<PortalState> = {}): PortalState => ({
  portal: name,
  enabled: true,
  fetchDetails: true,
  login: name === 'freelance' ? 'optional' : 'none',
  loginEnabled: false,
  signedIn: name === 'freelance' ? false : null,
  risk: name === 'freelancermap' ? 'low' : 'grey',
  health: { kind: 'ok' },
  quota: null,
  ...extra,
});

function lastRun(outcome: RunSummary['outcome'] = { kind: 'completed' }): RunSummary {
  return {
    run: 41,
    kind: 'fetch',
    outcome,
    dryRun: false,
    startedAt: at(1.2),
    finishedAt: at(1),
    scan: {
      mailsFound: 9,
      mailsChecked: 9,
      mailsDefective: 0,
      alertMails: 9,
      emptyAlerts: 1,
      postings: 14,
      new: 7,
      known: 6,
      dup: 1,
    },
    perPortal: [
      {
        portal: 'linkedin',
        new: 3,
        known: 2,
        dup: 1,
        fetched: 3,
        failed: 0,
        gone: 0,
        skipped: 0,
        stopped: null,
      },
      {
        portal: 'freelancermap',
        new: 3,
        known: 3,
        dup: 0,
        fetched: 3,
        failed: 0,
        gone: 0,
        skipped: 0,
        stopped: null,
      },
      {
        portal: 'freelance',
        new: 1,
        known: 1,
        dup: 0,
        fetched: 0,
        failed: 0,
        gone: 0,
        skipped: 1,
        stopped: null,
      },
    ],
    score: { scored: 12, excluded: 2, unscorable: 1, pending: 0, best: 91 },
    export: {
      overviewXlsx: 'C:/Users/demo/Jobs/Uebersicht.xlsx',
      overviewHtml: 'C:/Users/demo/Jobs/Uebersicht.html',
      backup: null,
      txtWritten: 7,
      txtFailed: 0,
      error: null,
    },
    emptyAlerts: [
      {
        portal: 'freelance',
        subject: 'Neue Projekte für Sie',
        date: at(20),
        gmailId: '18c2f0a9d1e4b7a3',
      },
    ],
  };
}

/* -------------------------------------------------------------------- store */

let jobs: JobView[] = [];
let state: AppState;

function initial(): void {
  jobs = scenario === 'many' ? manyJobs(2000) : sampleJobs();
  state = {
    platform: 'windows',
    dryRun: false,
    firstRun: false,
    running: null,
    settings: {
      workspace: 'C:/Users/demo/Documents/Job-Alerts',
      workspaceIsDefault: true,
      txtFiles: 38,
      excelExists: true,
    },
    mailbox: { user: 'alerts.demo@gmail.com', vault: 'windowsCredentialManager', error: null },
    profile: PROFILE,
    portals: [
      portal('linkedin'),
      portal('freelancermap', { quota: { usedHour: 9, capHour: 40, usedDay: 86, capDay: 100 } }),
      portal('freelance'),
    ],
    autoFetchOnStart: true,
    lastRun: lastRun(),
    counts: { new: 0, all: 0, excluded: 0, high: 0, noDetail: 0 },
    topMatches: [],
    matchPending: 0,
    dataDir: 'C:/Users/demo/AppData/Roaming/job-alert-monitor',
    logDir: 'C:/Users/demo/AppData/Roaming/job-alert-monitor/logs',
    resetReport: null,
  };
  switch (scenario) {
    case 'first-run':
      jobs = [];
      state.firstRun = true;
      state.mailbox = { user: null, vault: 'windowsCredentialManager', error: null };
      state.profile = null;
      state.lastRun = null;
      state.settings.excelExists = false;
      state.settings.txtFiles = 0;
      break;
    case 'mailbox-only':
      jobs = [];
      state.firstRun = true;
      state.profile = null;
      state.lastRun = null;
      break;
    case 'no-profile':
      state.profile = null;
      for (const j of jobs) j.match = null;
      break;
    case 'empty':
      jobs = [];
      state.lastRun = {
        ...lastRun(),
        perPortal: lastRun().perPortal.map((p) => ({ ...p, new: 0 })),
        emptyAlerts: [],
      };
      break;
    case 'offline':
      state.lastRun = lastRun({ kind: 'failed', error: { kind: 'mailConnect', params: {} } });
      break;
    case 'paused':
      state.portals[0]!.health = { kind: 'paused', until: later(95), reason: 'throttled' };
      state.portals[1]!.quota = { usedHour: 38, capHour: 40, usedDay: 97, capDay: 100 };
      state.portals[2]!.health = { kind: 'layoutSuspect', emptyMails: 2, pages: 0 };
      break;
    case 'reset':
      state.resetReport = { removed: 12, failed: 1 };
      break;
    case 'profile-broken':
      state.profile = {
        ...PROFILE,
        quality: null,
        understood: null,
        parseError: { kind: 'invalid', params: { reason: 'profileNotJson', line: 12, column: 3 } },
      };
      break;
    case 'running':
      state.running = {
        kind: 'fetch',
        startedAt: at(0.05),
        replay: [
          { type: 'progress', step: 'scan', portal: null, done: 9, total: 9 },
          { type: 'progress', step: 'fetch', portal: 'linkedin', done: 3, total: 5 },
          { type: 'progress', step: 'fetch', portal: 'freelancermap', done: 2, total: 2 },
          {
            type: 'portalHealth',
            portal: 'freelance',
            health: { kind: 'paused', until: later(12), reason: 'throttled' },
          },
          { type: 'status', code: 'waiting', portal: 'linkedin', until: later(0.7) },
        ],
      };
      break;
  }
  refresh();
}

function share(j: JobView): JobCounts {
  const out = j.match?.status === 'excluded';
  return {
    all: 1,
    new: j.unread && !out ? 1 : 0,
    excluded: out ? 1 : 0,
    high: j.match?.status === 'scored' && j.match.score >= 80 ? 1 : 0,
    noDetail: j.detail.kind !== 'ok' ? 1 : 0,
  };
}

function countsOf(list: JobView[]): JobCounts {
  const c: JobCounts = { new: 0, all: 0, excluded: 0, high: 0, noDetail: 0 };
  for (const j of list) {
    const s = share(j);
    c.all += s.all;
    c.new += s.new;
    c.excluded += s.excluded;
    c.high += s.high;
    c.noDetail += s.noDetail;
  }
  return c;
}

function refresh(): void {
  state.counts = countsOf(jobs);
  state.topMatches = jobs
    .filter((j) => j.match?.status === 'scored')
    .sort((a, b) => b.match!.score - a.match!.score)
    .slice(0, 5);
}

const fold = (text: string): string =>
  text
    .toLocaleLowerCase('de')
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '');

/** The same order and counts as store::job_page (one statement, list and counts agree). */
function listJobs(query: JobQuery): { jobs: JobView[]; counts: JobCounts } {
  if (scenario === 'list-error') throw fail('db');
  const needle = query.search ? fold(query.search) : null;
  const base = needle
    ? jobs.filter((j) => fold(`${j.title} ${j.company} ${j.location}`).includes(needle))
    : jobs;
  const isNew = (j: JobView): boolean => j.unread && j.match?.status !== 'excluded';
  const page = (query.facet === 'new' ? base.filter(isNew) : [...base]).sort((a, b) => {
    const ex = Number(a.match?.status === 'excluded') - Number(b.match?.status === 'excluded');
    if (ex !== 0) return ex;
    if (query.sort === 'match') {
      const na = Number(a.match === null) - Number(b.match === null);
      if (na !== 0) return na;
      const d = (b.match?.score ?? 0) - (a.match?.score ?? 0);
      if (d !== 0) return d;
    }
    return b.firstSeenAt.localeCompare(a.firstSeenAt) || a.key.id.localeCompare(b.key.id);
  });
  return {
    jobs: page.slice(query.offset, query.offset + Math.min(query.limit, 500)),
    counts: countsOf(base),
  };
}

/* ------------------------------------------------------------------- detail */

const AD_INTRO = (j: JobView): string =>
  `Für ${j.company} suchen wir ab sofort Unterstützung als ${j.title} in ${j.location || 'Deutschland'}.\n\n`;

function detailOf(j: JobView): JobDetail {
  const met = j.match?.top ?? [];
  const open = ['Erfahrung mit Power BI', 'Verhandlungssicheres Englisch'];
  const tasks = [
    'Aufbau und Weiterentwicklung des Reportings',
    'Führung eines Teams von sechs Personen',
  ];
  const parts: string[] = [AD_INTRO(j), 'Ihre Aufgaben\n'];
  for (const t of tasks) parts.push(`• ${t}\n`);
  parts.push('\nIhr Profil\n');
  for (const m of met) parts.push(`• ${m}\n`);
  for (const o of open) parts.push(`• ${o}\n`);
  parts.push('• Erfahrung mit Arbeitnehmerüberlassung von Vorteil\n');
  parts.push(
    '\nRahmen\nStart zum nächstmöglichen Zeitpunkt, Laufzeit sechs Monate mit Option auf Verlängerung. ',
  );
  parts.push('Tagessatz nach Absprache, Einsatz zu 60 Prozent remote.\n');
  const text = parts.join('');

  const reasons: Reason[] = [];
  const highlights: Highlight[] = [];
  const add = (
    kind: Reason['kind'],
    weight: Reason['weight'],
    code: string,
    label: string,
    profile: string | null,
    extra: Record<string, string | number | boolean | null> = {},
  ): void => {
    const id = String(reasons.length);
    const start = text.indexOf(label);
    const ranges = start >= 0 ? [{ start, end: start + label.length }] : [];
    for (const r of ranges) {
      highlights.push({
        id: String(highlights.length),
        start: r.start,
        end: r.end,
        kind,
        reason: id,
      });
    }
    reasons.push({
      id,
      kind,
      weight,
      code,
      label,
      evidence: profile
        ? { profile, path: 'kernkompetenzen[2].kompetenz', via: 'synonym', quote: label }
        : null,
      params: extra,
      ranges,
    });
  };
  met.forEach((m, i) =>
    add(
      'met',
      i === 0 ? 'must' : 'nice',
      'requirement',
      m,
      i === 0 ? 'Interim-Management' : 'Konzernabschluss nach HGB',
    ),
  );
  add('partial', 'must', 'requirement', tasks[0]!, 'Reporting');
  add('open', 'must', 'requirement', open[0]!, null);
  add('open', 'nice', 'requirement', open[1]!, null);
  add('check', 'info', 'startVague', 'Start zum nächstmöglichen Zeitpunkt', null);
  const excluded = j.match?.status === 'excluded';
  if (excluded) add('violation', 'hard', 'anue', 'Arbeitnehmerüberlassung', null);
  const criteria: Reason[] = [
    {
      id: 'c1',
      kind: 'met',
      weight: 'hard',
      code: 'minDayRate',
      label: '',
      evidence: null,
      params: {},
      ranges: [],
    },
    {
      id: 'c2',
      kind: 'met',
      weight: 'hard',
      code: 'countries',
      label: '',
      evidence: null,
      params: {},
      ranges: [],
    },
    {
      id: 'c3',
      kind: excluded ? 'violation' : 'check',
      weight: 'hard',
      code: 'noAnue',
      label: '',
      evidence: null,
      params: {},
      ranges: [],
    },
    {
      id: 'c4',
      kind: 'open',
      weight: 'hard',
      code: 'availability',
      label: '',
      evidence: null,
      params: {},
      ranges: [],
    },
  ];
  const ok = j.detail.kind === 'ok';
  return {
    job: j,
    text: ok ? text : j.detail.kind === 'teaser' ? AD_INTRO(j).trim() : null,
    url: `https://example.com/${j.portal}/${j.key.id}`,
    fetchedAt: ok ? j.firstSeenAt : null,
    mail: {
      subject: 'Neue Jobs für Ihr Profil',
      gmailUrl: 'https://mail.google.com/mail/u/0/#all/18c2f0a9d1e4b7a3',
    },
    match:
      j.match === null || state.profile === null
        ? null
        : {
            score: j.match.score,
            status: j.match.status,
            band: j.match.band,
            rev: '0123456789abcdef',
            at: at(1),
            summary: j.match.note,
            reasons: j.match.status === 'unscorable' ? [] : reasons,
            highlights: ok ? highlights : [],
            criteria,
          },
  };
}

/* --------------------------------------------------------------------- runs */

let running = false;

function fail(kind: ErrorInfo['kind'], params: ErrorInfo['params'] = {}): ErrorInfo {
  return { kind, params };
}

function emit(event: RunEvent): void {
  for (const channel of channels) channel.onmessage(event);
}

const NEW_JOBS: JobView[] = [
  job(
    'linkedin',
    '4100200399',
    'Interim CFO Carve-out',
    'Brückenwerk Industrie AG',
    'Hannover',
    0.1,
    { unread: true, detail: { kind: 'pending', retryAt: null } },
  ),
  job(
    'freelancermap',
    '2899',
    'Controlling Lead Post-Merger',
    'Elbufer Medien GmbH',
    'Hamburg',
    0.1,
    { unread: true, detail: { kind: 'pending', retryAt: null } },
  ),
  job(
    'freelance',
    '900499',
    'Buchhalter im Kundeneinsatz',
    'Personalwerk Nord GmbH',
    'Bremen',
    0.1,
    { unread: true, workMode: null, detail: { kind: 'pending', retryAt: null } },
  ),
];

function script(kind: RunSummary['kind']): RunEvent[] {
  const events: RunEvent[] = [
    { type: 'status', code: 'connectingMail', portal: null, until: null },
    { type: 'status', code: 'searchingMail', portal: null, until: null },
    { type: 'progress', step: 'scan', portal: null, done: 0, total: 3 },
    {
      type: 'alert',
      portal: 'linkedin',
      subject: 'Neue Jobs',
      date: at(0.1),
      postings: 1,
      gmailId: 'a1',
    },
    { type: 'progress', step: 'scan', portal: null, done: 1, total: 3 },
    {
      type: 'alert',
      portal: 'freelancermap',
      subject: 'Neue Projekte',
      date: at(0.1),
      postings: 1,
      gmailId: 'a2',
    },
    { type: 'progress', step: 'scan', portal: null, done: 2, total: 3 },
    {
      type: 'alert',
      portal: 'freelance',
      subject: 'Projekte',
      date: at(0.1),
      postings: 1,
      gmailId: 'a3',
    },
    { type: 'progress', step: 'scan', portal: null, done: 3, total: 3 },
    ...NEW_JOBS.map((j): RunEvent => ({ type: 'jobUpdated', job: j })),
    { type: 'status', code: 'fetchingDetails', portal: 'linkedin', until: null },
    { type: 'progress', step: 'fetch', portal: 'linkedin', done: 0, total: 1 },
    { type: 'progress', step: 'fetch', portal: 'freelancermap', done: 0, total: 1 },
    { type: 'progress', step: 'fetch', portal: 'linkedin', done: 1, total: 1 },
    { type: 'progress', step: 'fetch', portal: 'freelancermap', done: 1, total: 1 },
    {
      type: 'portalHealth',
      portal: 'freelance',
      health: { kind: 'paused', until: later(15), reason: 'throttled' },
    },
    { type: 'status', code: 'waiting', portal: 'freelance', until: later(0.5) },
    { type: 'status', code: 'scoring', portal: null, until: null },
    { type: 'progress', step: 'score', portal: null, done: 0, total: 3 },
  ];
  const results: Match[] = [
    scored(88, ['Carve-out Erfahrung', 'Konzernabschluss nach HGB'], 4, 4),
    scored(61, ['Post-Merger-Integration'], 2, 4),
    excludedBy('noAnue', 49),
  ];
  NEW_JOBS.forEach((j, i) => {
    events.push({
      type: 'jobUpdated',
      job: { ...j, detail: i === 2 ? j.detail : { kind: 'ok' }, match: results[i]! },
    });
    events.push({ type: 'progress', step: 'score', portal: null, done: i + 1, total: 3 });
  });
  events.push({ type: 'status', code: 'writingFiles', portal: null, until: null });
  events.push({ type: 'progress', step: 'export', portal: null, done: 1, total: 1 });
  events.push({
    type: 'finished',
    summary: {
      ...lastRun(),
      kind,
      startedAt: at(0.05),
      finishedAt: at(0),
      perPortal: [
        {
          portal: 'linkedin',
          new: 1,
          known: 0,
          dup: 0,
          fetched: 1,
          failed: 0,
          gone: 0,
          skipped: 0,
          stopped: null,
        },
        {
          portal: 'freelancermap',
          new: 1,
          known: 0,
          dup: 0,
          fetched: 1,
          failed: 0,
          gone: 0,
          skipped: 0,
          stopped: null,
        },
        {
          portal: 'freelance',
          new: 1,
          known: 0,
          dup: 0,
          fetched: 0,
          failed: 0,
          gone: 0,
          skipped: 1,
          stopped: { kind: 'paused', until: later(15), reason: 'throttled' },
        },
      ],
      emptyAlerts: [],
    },
  });
  return events;
}

function startRun(kind: RunSummary['kind']): void {
  if (running) throw fail('busy');
  if (state.mailbox.user === null) throw fail('mailMissing');
  running = true;
  harness.done = false;
  const events = scenario === 'offline' ? offlineScript() : script(kind);
  let index = 0;
  const step = (): void => {
    if (!running) return;
    const event = events[index++];
    if (event === undefined) return;
    apply(event);
    emit(event);
    if (event.type === 'finished') {
      running = false;
      harness.done = true;
      return;
    }
    setTimeout(step, TICK);
  };
  setTimeout(step, TICK);
}

function offlineScript(): RunEvent[] {
  return [
    { type: 'status', code: 'connectingMail', portal: null, until: null },
    {
      type: 'finished',
      summary: {
        ...lastRun({ kind: 'failed', error: fail('mailConnect') }),
        perPortal: [],
        emptyAlerts: [],
      },
    },
  ];
}

/** Keep the store in step with the events, so a reload after the run sees the new jobs. */
function apply(event: RunEvent): void {
  if (event.type === 'jobUpdated') {
    const i = jobs.findIndex(
      (j) => j.key.portal === event.job.key.portal && j.key.id === event.job.key.id,
    );
    if (i >= 0) jobs[i] = event.job;
    else jobs.unshift(event.job);
    refresh();
  } else if (event.type === 'finished') {
    state.firstRun = false;
    state.lastRun = event.summary;
    state.running = null;
  }
}

function cancelRun(): void {
  if (!running) return;
  running = false;
  const summary: RunSummary = { ...lastRun({ kind: 'cancelled' }), perPortal: [], emptyAlerts: [] };
  setTimeout(() => {
    const event: RunEvent = { type: 'finished', summary };
    apply(event);
    emit(event);
    harness.done = true;
  }, TICK);
}

/* ----------------------------------------------------------------- handlers */

type Args<K extends keyof Commands> = Omit<Commands[K]['args'], 'channel'>;
type Handlers = { [K in keyof Commands]: (args: Args<K>) => Commands[K]['result'] };

const find = (key: { portal: string; id: string }): JobView | undefined =>
  jobs.find((j) => j.key.portal === key.portal && j.key.id === key.id);

const handlers: Handlers = {
  app_state: () => structuredClone(state),
  start_run: ({ request }) => {
    startRun(request.kind);
    return null;
  },
  cancel_run: () => {
    cancelRun();
    return null;
  },
  list_jobs: ({ query }) => structuredClone(listJobs(query)),
  job_detail: ({ key }) => {
    const j = find(key);
    if (j === undefined) throw fail('notFound');
    return structuredClone(detailOf(j));
  },
  mark_read: ({ key }) => {
    const j = find(key);
    if (j === undefined || !j.unread) return false;
    j.unread = false;
    refresh();
    return true;
  },
  set_pinned: ({ key, on }) => {
    const j = find(key);
    if (j === undefined || j.pinned === on) return false;
    j.pinned = on;
    return true;
  },
  pick_profile: () => {
    state.profile = PROFILE;
    if (jobs.every((j) => j.match === null)) {
      const sample = sampleJobs();
      for (const j of jobs) j.match = sample.find((s) => s.key.id === j.key.id)?.match ?? null;
    }
    refresh();
    return PROFILE;
  },
  remove_profile: () => {
    state.profile = null;
    return true;
  },
  save_profile_template: () => 'C:/Users/demo/Documents/Job-Alerts/profil-vorlage.json',
  save_mailbox: ({ user, password }) => {
    if (!/^[^@\s]+@[^@\s]+\.[a-z]{2,}$/i.test(user)) {
      throw fail('invalid', { reason: 'mailAddress' });
    }
    if (!/^[a-z]{16}$/i.test(password.replace(/\s/g, ''))) {
      throw fail('invalid', { reason: 'appPassword' });
    }
    state.mailbox = { user, vault: 'windowsCredentialManager', error: null };
    return state.mailbox;
  },
  remove_mailbox: () => {
    state.mailbox = { user: null, vault: 'windowsCredentialManager', error: null };
    return true;
  },
  portal_login: ({ portal: name }) => {
    const p = state.portals.find((x) => x.portal === name);
    if (p) p.signedIn = true;
    return true;
  },
  portal_logout: ({ portal: name }) => {
    const p = state.portals.find((x) => x.portal === name);
    if (p) p.signedIn = false;
    return true;
  },
  pick_workspace: () => null,
  rewrite_txt: () => ({
    overviewXlsx: null,
    overviewHtml: null,
    backup: null,
    txtWritten: state.settings.txtFiles,
    txtFailed: 0,
    error: null,
  }),
  clear_txt: () => {
    const removed = state.settings.txtFiles;
    state.settings.txtFiles = 0;
    return { removed, failed: [] };
  },
  open_target: () => null,
  save_settings: ({ patch }) => {
    for (const change of patch.portals) {
      const p = state.portals.find((x) => x.portal === change.portal);
      if (p === undefined) continue;
      if (change.enabled !== null) p.enabled = change.enabled;
      if (change.fetchDetails !== null) p.fetchDetails = change.fetchDetails;
      if (change.loginEnabled !== null) {
        p.loginEnabled = change.loginEnabled;
        p.risk = change.loginEnabled ? 'account' : 'grey';
      }
    }
    if (state.portals.every((p) => !p.enabled)) throw fail('invalid', { reason: 'noPortal' });
    if (patch.autoFetchOnStart !== null) state.autoFetchOnStart = patch.autoFetchOnStart;
    return structuredClone(state);
  },
  reset_all: () => null,
  report_ui_error: () => null,
};

const harness: Harness = {
  calls: [],
  emit(event) {
    apply(event);
    emit(event);
  },
  done: false,
};
window.__harness = harness;
initial();

/* --------------------------------------------------------------------- core */

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  harness.calls.push([command, args]);
  if (args.channel instanceof Channel) channels.add(args.channel as Channel<RunEvent>);
  const handler = handlers[command as keyof Commands] as ((a: unknown) => unknown) | undefined;
  if (handler === undefined) throw fail('internal', { command });
  if (DELAY > 0 && command !== 'report_ui_error') {
    await new Promise((resolve) => setTimeout(resolve, DELAY));
  }
  return handler(args) as T;
}

/* ------------------------------------------------------------------- window */

type ResizeHandler = () => void;

class FakeWindow {
  #maximized = false;
  #resized = new Set<ResizeHandler>();

  async minimize(): Promise<void> {
    harness.calls.push(['window.minimize', null]);
  }

  async toggleMaximize(): Promise<void> {
    harness.calls.push(['window.toggleMaximize', null]);
    this.#maximized = !this.#maximized;
    for (const handler of this.#resized) handler();
  }

  async close(): Promise<void> {
    harness.calls.push(['window.close', null]);
  }

  async startDragging(): Promise<void> {
    harness.calls.push(['window.startDragging', null]);
  }

  async isMaximized(): Promise<boolean> {
    return this.#maximized;
  }

  async onResized(handler: ResizeHandler): Promise<() => void> {
    this.#resized.add(handler);
    return () => this.#resized.delete(handler);
  }
}

const fakeWindow = new FakeWindow();

export function getCurrentWindow(): FakeWindow {
  return fakeWindow;
}
