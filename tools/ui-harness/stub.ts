// Typed stand-in for @tauri-apps/api in the harness build (`vite build --mode harness`
// aliases every `@tauri-apps/api/*` import to this file). It answers every IPC command
// (typed by the generated `Commands` map) from a small in-memory store with realistic,
// invented sample data, records the calls and lets a test push run events:
//
//   window.__harness.calls          [command, args][]
//   window.__harness.emit(event)    send a RunEvent the way Rust does (the run's channel,
//                                   else the page's channel from its last app_state)
//   window.__harness.appRun(kind)   a run the app starts by itself (the auto fetch, a
//                                   rescore after a profile change): on the page's channel
//   window.__harness.fire(name, p)  an app event, as Rust's `window.emit` sends it
//   window.__harness.done           true once a started run has finished
//   window.__harness.detailDelay    ms `job_detail` takes (default 0)
//   window.__harness.failPages      so many next `list_jobs` calls for a later page fail
//   window.__harness.holdAfter      a scripted run pauses after so many events (null = on)
//   window.__harness.job(key)       a copy of a job as the stub holds it
//
// Scenarios (`?scenario=`): default · first-run · mailbox-only · no-profile · empty ·
// many (2000 jobs) · offline · paused · running · slow · list-error · profile-broken ·
// profile-thin · reset (the state after
// "reset everything": first run, no mailbox, no profile, the report) · first-run-empty-profile
// · dry-run (the demo: a Probelauf mailbox, every command that writes outside the database
// refuses with `dryRun` like `ensure_real`).
// `save_mailbox` refuses the app password `falschfalschfals` with `mailAuth` (Gmail said no).
// `?tick=ms` sets the pace of a scripted run (default 40); `?export=locked` lets the export
// of a run find the Excel file open; `?mail=offline` lets every fetch fail to reach Gmail.
// Dates are fixed so screenshots stay stable (the tests also fix the clock). The portals
// come in the order of the backend (`Portal::ALL`).

import type {
  AppState,
  Commands,
  ErrorInfo,
  Highlight,
  JobCounts,
  JobDetail,
  JobKey,
  JobQuery,
  JobView,
  Portal,
  PortalState,
  ProfileDraft,
  ProfileForm,
  ProfileInfo,
  Reason,
  RunEvent,
  RunRequest,
  RunSummary,
} from '../../ui/src/lib/ipc/types';

interface Harness {
  calls: [string, unknown][];
  emit: (event: RunEvent) => void;
  /** A run the app starts by itself, on the page's channel (no `start_run`). */
  appRun: (kind: RunSummary['kind']) => void;
  /** An app event the way `window.emit` sends it (the native menu's `navigate`). */
  fire: (name: string, payload: unknown) => void;
  done: boolean;
  /** Milliseconds `job_detail` takes. */
  detailDelay: number;
  /** So many next `list_jobs` calls for a later page (offset > 0) fail. */
  failPages: number;
  /** A scripted run pauses after so many of its events until this is null again. */
  holdAfter: number | null;
  /** A copy of a job as the stub holds it (null if unknown). */
  job: (key: JobKey) => JobView | null;
}

declare global {
  interface Window {
    __harness: Harness;
  }
}

/* ------------------------------------------------------------------ channel */

/**
 * Tauri's Channel as the page sees it (@tauri-apps/api/core): messages carry the index of
 * their Rust-side sender and are delivered in that order; the message `end` (the Rust side
 * dropped its channel) unregisters the callback, after which nothing arrives any more.
 */
export class Channel<T = unknown> {
  onmessage: (message: T) => void = () => undefined;
  #next = 0;
  #pending = new Map<number, T>();
  #end: number | null = null;
  #closed = false;

  /** What `window.__TAURI_INTERNALS__.runCallback` does with one raw message. */
  receive(raw: { index: number; message?: T; end?: true }): void {
    if (this.#closed) return;
    if (raw.end) {
      if (raw.index === this.#next) this.#closed = true;
      else this.#end = raw.index;
      return;
    }
    if (raw.index !== this.#next) {
      this.#pending.set(raw.index, raw.message as T);
      return;
    }
    this.onmessage(raw.message as T);
    this.#next += 1;
    while (this.#pending.has(this.#next)) {
      const message = this.#pending.get(this.#next) as T;
      this.#pending.delete(this.#next);
      this.onmessage(message);
      this.#next += 1;
    }
    if (this.#next === this.#end) this.#closed = true;
  }
}

/**
 * The Rust side of one `channel` argument (tauri::ipc::Channel): its own message counter
 * from 0, shared by its clones; once the last clone is dropped it sends `end`. Delivery is
 * asynchronous, as with `webview.eval`.
 */
class Sender {
  #index = 0;
  #holders = 0;

  constructor(readonly channel: Channel<RunEvent>) {}

  hold(): Sender {
    this.#holders += 1;
    return this;
  }

  send(event: RunEvent): void {
    const index = this.#index++;
    const message = structuredClone(event);
    queueMicrotask(() => this.channel.receive({ index, message }));
  }

  release(): void {
    this.#holders -= 1;
    if (this.#holders > 0) return;
    const index = this.#index;
    queueMicrotask(() => this.channel.receive({ index, end: true }));
  }
}

/** The page's channel from its last `app_state` (commands::scoring keeps it). */
let pageSender: Sender | null = null;
/** The channel of the run in progress (commands::run::RunHandle). */
let runSender: Sender | null = null;

/* ----------------------------------------------------------------- scenario */

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'default';
const TICK = Number(params.get('tick') ?? 40);
const DELAY = scenario === 'slow' ? 900 : 0;
const EXPORT_LOCKED = params.get('export') === 'locked';
const MAIL_OFFLINE = scenario === 'offline' || params.get('mail') === 'offline';
/** The order of the backend (`Portal::ALL`), on every screen. */
const PORTALS: readonly Portal[] = ['linkedin', 'freelance', 'freelancermap'];

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

/** Excluded by a hard criterion: the engine names the first violation's reason code. */
const excludedBy = (
  code: string,
  score: number,
  params: Record<string, string | number> = {},
): Match => ({
  score,
  band: score >= 80 ? 'high' : score >= 40 ? 'mid' : 'low',
  status: 'excluded',
  note: { code, params },
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
    appStatus: null,
    hidden: false,
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
        match: excludedBy('anue', 55),
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
        appStatus: 'interview',
      },
    ),
    job('freelancermap', '2804', 'Interim Treasury Manager', 'Rheinhafen Chemie GmbH', 'Köln', 30, {
      match: scored(47, ['Liquiditätsplanung'], 1, 3),
      appStatus: 'applied',
    }),
    // "Not interesting": in no list but the hidden one and in no count but its own.
    job(
      'linkedin',
      '4100200306',
      'Sachbearbeitung Kreditoren',
      'Nordhafen Logistik GmbH',
      'Bremen',
      40,
      {
        match: scored(18, [], 0, 4),
        hidden: true,
      },
    ),
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
      match: excludedBy('country', 38, { allowed: 'DE, AT' }),
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
  const portals = PORTALS;
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
              ? excludedBy('dayRate', score, { rate: 700, min: 1100 })
              : scored(score, ['Controlling im Konzern']),
        },
      ),
    );
  }
  return out;
}

/** The invented sample profile of the fixtures (core/tests/fixtures/matching/sample_profile.json). */
const row = (name: string, years: number | null, aliases: string[], origin: number) => ({
  name,
  years,
  aliases,
  origin,
});
const PROFILE_FORM: ProfileForm = {
  name: 'Erika Beispiel',
  title: 'Interim Managerin Finanzen',
  competences: [
    row('Interim Management', 12, [], 0),
    row('Controlling', 18, ['Financial Controlling', 'FP&A'], 1),
    row('Konzernrechnungslegung nach IFRS', 14, [], 2),
    row('Konsolidierung', 11, [], 3),
    row('Liquiditätsplanung', 10, [], 4),
    row('Restrukturierung', 8, ['Sanierung'], 5),
  ],
  strengths: ['Aufbau von Konzernreportings in weniger als 100 Tagen'],
  keywords: ['IFRS', 'HGB', 'Konzernabschluss'],
  years: 20,
  degrees: ['Diplom-Kauffrau (Univ.)'],
  industries: ['Maschinenbau', 'Automotive', 'Chemie'],
  tools: ['SAP S/4HANA', 'LucaNet', 'Power BI'],
  certificates: ['Certified Interim Manager (DDIM)'],
  languages: [
    { language: 'Deutsch', level: 'native', origin: 0 },
    { language: 'Englisch', level: 'b2', origin: 1 },
  ],
  focus: ['Controlling', 'Konzernrechnungslegung nach IFRS'],
  roles: ['Interim CFO'],
  wishes: { dayRate: 1200, remote: 'mostly', regions: ['Hamburg'], industries: [] },
  criteria: {
    minDayRate: 1100,
    countries: ['DE', 'AT'],
    noAnue: true,
    available: { kind: 'unset' },
    remoteOutside: true,
    targetYears: 15,
    minSalary: null,
    permanentPlaces: [],
    permanentRemoteMin: null,
  },
};

/** The IT sample of the fixtures (sample_profile_it.json) as a chosen file. */
const FILE_DRAFT: ProfileDraft = {
  form: {
    ...structuredClone(PROFILE_FORM),
    name: 'Jonas Muster',
    title: '',
    competences: [
      row('SAP-Projektleitung', 12, [], 0),
      row('SAP S/4HANA Migration', 6, [], 1),
      row('Programmmanagement', 8, [], 2),
    ],
    focus: [],
    roles: [],
    wishes: { dayRate: null, remote: 'partly', regions: [], industries: [] },
  },
  source: '{"name": "Jonas Muster"}',
  quality: 'thin',
};

const PROFILE: ProfileInfo = {
  fileName: 'profil-interim-finance.json',
  bytes: 18_422,
  savedAt: at(72),
  quality: 'good',
  understood: {
    competenceCount: 42,
    packs: ['finance', 'sap'],
    years: 28,
    degrees: ['Diplom-Kauffrau (Univ.)'],
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
      { code: 'minDayRate', params: { set: true, min: '1100' } },
      { code: 'countries', params: { set: true, countries: 'DE, AT' } },
      { code: 'noAnue', params: { set: true } },
      { code: 'availability', params: { set: false, from: null } },
      { code: 'minSalary', params: { set: false, min: null } },
      { code: 'permanentRegion', params: { set: false, places: null, remoteMin: null } },
      { code: 'targetYears', params: { set: true, min: 15 } },
    ],
    warnings: [
      {
        code: 'criterionNotUnderstood',
        params: { key: 'festanstellung_remote_min', value: '"viel"' },
      },
    ],
    focus: PROFILE_FORM.focus,
    roles: PROFILE_FORM.roles,
    wishes: PROFILE_FORM.wishes,
  },
  scoredAt: at(1),
  pending: 0,
  parseError: null,
  form: PROFILE_FORM,
};

/** The request for Claude (the real text lives in core/src/profile/prompt.rs). */
const PROMPT = 'Erstelle aus meinem angehängten Lebenslauf ein Beraterprofil.';

type Json = Record<string, unknown>;
const texts = (value: unknown, key?: string): string[] =>
  (Array.isArray(value) ? value : [])
    .map((item) =>
      typeof item === 'string'
        ? item
        : key && item && typeof item === 'object'
          ? (item as Json)[key]
          : null,
    )
    .filter((item): item is string => typeof item === 'string' && item.trim() !== '');
const LEVELS: Record<string, ProfileForm['languages'][number]['level']> = {
  a1: 'a1',
  a2: 'a2',
  b1: 'b1',
  b2: 'b2',
  c1: 'c1',
  c2: 'c2',
  muttersprache: 'native',
};

/** Claude's answer as the backend reads it: the JSON (also in a code block) into the form. */
function answerDraft(answer: string): ProfileDraft {
  const fenced = /```[a-z]*\s*([\s\S]*?)```/.exec(answer)?.[1];
  const text = fenced ?? answer.slice(answer.indexOf('{'), answer.lastIndexOf('}') + 1);
  let data: Json;
  try {
    data = JSON.parse(text) as Json;
  } catch {
    throw fail('invalid', { reason: 'profileAnswer' });
  }
  const list = (key: string): unknown[] =>
    Array.isArray(data[key]) ? (data[key] as unknown[]) : [];
  const form: ProfileForm = {
    ...structuredClone(PROFILE_FORM),
    name: typeof data.name === 'string' ? data.name : '',
    title: typeof data.titel === 'string' ? data.titel : '',
    competences: list('kernkompetenzen').map((item, index) => {
      const entry = item as Json;
      return row(
        String(entry.kompetenz ?? ''),
        typeof entry.jahre === 'number' ? entry.jahre : null,
        texts(entry.auch),
        index,
      );
    }),
    strengths: texts(data.alleinstellungsmerkmale),
    keywords: texts(data.keywords),
    years: typeof data.berufserfahrung_jahre === 'number' ? data.berufserfahrung_jahre : null,
    degrees: texts(data.ausbildung, 'abschluss'),
    industries: texts(data.branchen, 'branche'),
    tools: texts(data.methoden_tools, 'name'),
    certificates: texts(data.zertifizierungen, 'name'),
    languages: list('sprachen').map((item, index) => {
      const entry = item as Json;
      return {
        language: String(entry.sprache ?? ''),
        level: LEVELS[String(entry.niveau ?? '').toLowerCase()] ?? null,
        origin: index,
      };
    }),
    focus: texts(data.schwerpunkte),
    roles: [],
    wishes: { dayRate: null, remote: null, regions: [], industries: [] },
    criteria: {
      ...PROFILE_FORM.criteria,
      minDayRate: null,
      countries: [],
      noAnue: false,
      targetYears: null,
    },
  };
  if (form.competences.length === 0 && form.name === '')
    throw fail('invalid', { reason: 'profileAnswer' });
  return { form, source: text, quality: form.competences.length >= 5 ? 'good' : 'thin' };
}

/** A saved form: trimmed, empty rows gone, origins as the backend reads them back. */
function savedForm(form: ProfileForm): ProfileForm {
  const clean = (items: string[]): string[] => items.map((t) => t.trim()).filter((t) => t !== '');
  return {
    ...structuredClone(form),
    name: form.name.trim(),
    title: form.title.trim(),
    competences: form.competences
      .filter((r) => r.name.trim() !== '')
      .map((r, index) => ({ ...r, name: r.name.trim(), aliases: clean(r.aliases), origin: index })),
    languages: form.languages
      .filter((r) => r.language.trim() !== '')
      .map((r, index) => ({ ...r, language: r.language.trim(), origin: index })),
    focus: clean(form.focus),
  };
}

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
        new: 2,
        known: 2,
        dup: 1,
        fetched: 1,
        failed: 0,
        gone: 0,
        skipped: 1,
        stopped: null,
      },
      {
        portal: 'freelance',
        new: 2,
        known: 1,
        dup: 0,
        fetched: 2,
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
    ],
    // Seven new, one of them excluded; two of the others fit well.
    newJobs: { count: 6, high: 2 },
    score: { scored: 10, excluded: 2, unscorable: 1, pending: 1, best: 91 },
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
      portal('freelance'),
      portal('freelancermap', { quota: { usedHour: 9, capHour: 40, usedDay: 86, capDay: 100 } }),
    ],
    autoFetchOnStart: true,
    lastRun: lastRun(),
    counts: countsOf([]),
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
    case 'no-files':
      // A connected mailbox, but nothing written to the workspace yet.
      state.settings.excelExists = false;
      state.settings.txtFiles = 0;
      break;
    case 'mailbox-only':
      jobs = [];
      state.firstRun = true;
      state.profile = null;
      state.lastRun = null;
      break;
    case 'first-run-empty-profile':
      // A profile that names no competences: nothing can be scored with it.
      jobs = [];
      state.firstRun = true;
      state.lastRun = null;
      state.profile = { ...PROFILE, quality: 'empty' };
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
        newJobs: { count: 0, high: 0 },
        emptyAlerts: [],
      };
      break;
    case 'offline':
      state.lastRun = lastRun({ kind: 'failed', error: { kind: 'mailConnect', params: {} } });
      break;
    case 'paused':
      state.portals[0]!.health = { kind: 'paused', until: later(95), reason: 'throttled' };
      state.portals[1]!.health = { kind: 'layoutSuspect', emptyMails: 2, pages: 0 };
      // The hour binds: the bar and its words both speak of the hour.
      state.portals[2]!.quota = { usedHour: 38, capHour: 40, usedDay: 61, capDay: 100 };
      break;
    case 'reset':
      // After "reset everything" the app starts empty: the first-run page, with the report.
      jobs = [];
      state.firstRun = true;
      state.mailbox = { user: null, vault: 'windowsCredentialManager', error: null };
      state.profile = null;
      state.lastRun = null;
      state.settings.excelExists = false;
      state.settings.txtFiles = 0;
      state.resetReport = { removed: 12, failed: 1 };
      break;
    case 'dry-run':
      state.dryRun = true;
      state.mailbox = {
        user: 'probelauf@example.org',
        vault: 'windowsCredentialManager',
        error: null,
      };
      break;
    case 'profile-broken':
      state.profile = {
        ...PROFILE,
        quality: null,
        understood: null,
        parseError: { kind: 'invalid', params: { reason: 'profileNotJson', line: 12, column: 3 } },
        form: null,
      };
      break;
    case 'profile-thin':
      state.profile = {
        ...PROFILE,
        quality: 'thin',
        understood: {
          ...PROFILE.understood!,
          competenceCount: 3,
          competences: ['Controlling', 'Treasury', 'IFRS'],
          packs: ['finance'],
          focus: [],
          roles: [],
          warnings: [{ code: 'fewCompetences', params: { count: 3 } }],
        },
        form: {
          ...structuredClone(PROFILE_FORM),
          competences: [row('Controlling', 18, [], 0), row('Treasury', null, [], 1)],
          focus: [],
          roles: [],
          keywords: ['IFRS'],
          tools: [],
          certificates: [],
          languages: [],
          wishes: { dayRate: null, remote: null, regions: [], industries: [] },
        },
      };
      break;
    case 'running':
      state.running = {
        kind: 'fetch',
        startedAt: at(0.05),
        replay: [
          { type: 'progress', step: 'scan', portal: null, done: 9, total: 9 },
          { type: 'progress', step: 'fetch', portal: null, done: 5, total: 7 },
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

/**
 * The counts of store::job_page: "Neu" is unread and not excluded, per portal too; a hidden
 * job is only in "hidden".
 */
function countsOf(list: JobView[]): JobCounts {
  const c: JobCounts = {
    new: 0,
    all: 0,
    excluded: 0,
    high: 0,
    noDetail: 0,
    pinned: 0,
    applications: 0,
    hidden: 0,
    newByPortal: PORTALS.map((portal) => ({ portal, new: 0 })),
  };
  for (const j of list) {
    if (j.hidden) {
      c.hidden += 1;
      continue;
    }
    const out = j.match?.status === 'excluded';
    const isNew = j.unread && !out;
    c.all += 1;
    c.new += isNew ? 1 : 0;
    c.excluded += out ? 1 : 0;
    c.high += j.match?.status === 'scored' && j.match.score >= 80 ? 1 : 0;
    c.noDetail += j.detail.kind !== 'ok' ? 1 : 0;
    c.pinned += j.pinned ? 1 : 0;
    c.applications += j.appStatus !== null ? 1 : 0;
    const line = c.newByPortal.find((p) => p.portal === j.portal);
    if (line && isNew) line.new += 1;
  }
  return c;
}

/* ------------------------------------------------------------------- marks */

/** When a job's status was set, its note and when it was hidden (store::marks). */
const statusAt = new Map<string, string>([
  ['linkedin:4100200303', at(5)],
  ['freelancermap:2804', at(20)],
]);
const notes = new Map<string, string>([['linkedin:4100200303', 'Zweites Gespräch am Freitag.']]);
const hiddenAt = new Map<string, string>([['linkedin:4100200306', at(30)]]);
const markKey = (key: JobKey): string => `${key.portal}:${key.id}`;

/** The list of a facet (store::ListFacet): a hidden job is in "hidden" only. */
function inFacet(j: JobView, facet: JobQuery['facet']): boolean {
  switch (facet) {
    case 'new':
      return !j.hidden && j.unread;
    case 'all':
      return !j.hidden;
    case 'applications':
      return !j.hidden && j.appStatus !== null;
    case 'hidden':
      return j.hidden;
  }
}

function refresh(): void {
  state.counts = countsOf(jobs);
}

const fold = (text: string): string =>
  text
    .toLocaleLowerCase('de')
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '');

/** The same order and counts as store::job_page (one statement, list and counts agree). */
function listJobs(query: JobQuery): { jobs: JobView[]; counts: JobCounts } {
  if (scenario === 'list-error') throw fail('db');
  if (query.offset > 0 && harness.failPages > 0) {
    harness.failPages -= 1;
    throw fail('db');
  }
  const needle = query.search ? fold(query.search) : null;
  const base = needle
    ? jobs.filter((j) => fold(`${j.title} ${j.company} ${j.location}`).includes(needle))
    : jobs;
  // Neu lists every unread job, excluded ones too (grey behind the divider); only the count
  // leaves them out (store::job_page). Applications follow their latest change, hidden jobs
  // the moment they were hidden.
  const latest = (times: Map<string, string>) => (a: JobView, b: JobView) =>
    (times.get(markKey(b.key)) ?? '').localeCompare(times.get(markKey(a.key)) ?? '') ||
    a.key.id.localeCompare(b.key.id);
  const listed = base.filter((j) => inFacet(j, query.facet));
  if (query.facet === 'applications' || query.facet === 'hidden') {
    listed.sort(latest(query.facet === 'hidden' ? hiddenAt : statusAt));
    return {
      jobs: listed.slice(query.offset, query.offset + Math.min(query.limit, 500)),
      counts: countsOf(base),
    };
  }
  const page = listed.sort((a, b) => {
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

/** Requirements the stub ads ask for, beyond the job's own top reasons. */
const MORE_MUSTS = [
  'Führung eines Finanzteams',
  'Erfahrung mit Abschlussprüfungen',
  'Reporting nach IFRS',
  'Verhandlungssicheres Deutsch',
];
const OPEN_MUSTS = ['Erfahrung mit Power BI', 'Kenntnisse in LucaNet', 'Branchenerfahrung Energie'];
const PARTIAL_MUST = 'Aufbau und Weiterentwicklung des Reportings';
const NICE_MET = 'Konzernabschluss nach HGB';
const NICE_OPEN = 'Verhandlungssicheres Englisch';

/** Contract type per job: interim unless the title says otherwise (one inferred). */
function contractOf(j: JobView): { type: string; inferred: boolean } {
  if (j.key.id === '4100200304') return { type: 'permanent', inferred: false };
  if (j.key.id === '900412') return { type: 'anue', inferred: false };
  if (j.key.id === '4100200303') return { type: 'permanent', inferred: true };
  return { type: 'interim', inferred: false };
}

/**
 * The detail of a job. Its reasons agree with the list numbers: exactly `mustMet` met must
 * requirements, and `mustTotal - mustMet` that are not met (the first of two or more is
 * partial, the rest open), plus one met and one open nice-to-have.
 */
function detailOf(j: JobView): JobDetail {
  const m = j.match;
  const mustMet = m?.mustMet ?? 0;
  const missing = Math.max(0, (m?.mustTotal ?? 0) - mustMet);
  const metMusts = [...(m?.top.slice(0, 1) ?? []), ...MORE_MUSTS].slice(0, mustMet);
  const partial = missing >= 2 ? [PARTIAL_MUST] : [];
  const openMusts = OPEN_MUSTS.slice(0, missing - partial.length);
  const tasks = ['Führung eines Teams von sechs Personen', 'Monatsabschluss und Forecast'];

  const parts: string[] = [AD_INTRO(j), 'Ihre Aufgaben\n'];
  for (const t of [...tasks, ...partial]) parts.push(`• ${t}\n`);
  parts.push('\nIhr Profil\n');
  for (const r of [...metMusts, NICE_MET, ...openMusts, NICE_OPEN]) parts.push(`• ${r}\n`);
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
    const start = label ? text.indexOf(label) : -1;
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
  const contract = contractOf(j);
  add(contract.type === 'interim' ? 'met' : 'partial', 'info', 'contractType', '', null, contract);
  metMusts.forEach((r, i) =>
    add('met', 'must', 'requirement', r, i === 0 ? 'Interim-Management' : 'Controlling'),
  );
  for (const r of partial) add('partial', 'must', 'requirement', r, 'Reporting');
  for (const r of openMusts) add('open', 'must', 'requirement', r, null);
  add('met', 'nice', 'requirement', NICE_MET, 'Konzernabschluss nach HGB');
  add('open', 'nice', 'requirement', NICE_OPEN, null);
  add('check', 'info', 'startVague', 'Start zum nächstmöglichen Zeitpunkt', null);
  const excluded = m?.status === 'excluded';
  if (excluded) add('violation', 'hard', 'anue', 'Arbeitnehmerüberlassung', null);
  // The strip shows the criteria the profile sets (the engine leaves out the others).
  const criterion = (id: string, kind: Reason['kind'], code: string): Reason => ({
    id,
    kind,
    weight: 'hard',
    code,
    label: '',
    evidence: null,
    params: {},
    ranges: [],
  });
  const criteria: Reason[] = [
    criterion('c:minDayRate', 'met', 'minDayRate'),
    criterion('c:countries', 'met', 'countries'),
    criterion('c:noAnue', excluded ? 'violation' : 'check', 'noAnue'),
    criterion('c:targetYears', 'met', 'targetYears'),
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
      m === null || state.profile === null
        ? null
        : {
            score: m.score,
            status: m.status,
            band: m.band,
            rev: '0123456789abcdef',
            at: at(1),
            summary: m.note,
            reasons: m.status === 'unscorable' ? [] : reasons,
            highlights: ok ? highlights : [],
            criteria,
          },
    note: notes.get(markKey(j.key)) ?? null,
    appStatusAt: j.appStatus === null ? null : (statusAt.get(markKey(j.key)) ?? at(0)),
  };
}

/** The profile part of the prompts (core leaves out name and contact data the same way). */
const PROMPT_PROFILE = [
  'Mein Profil (JSON, ohne Name und Kontaktdaten)',
  '```json',
  JSON.stringify({ kernkompetenzen: PROFILE.understood?.competences ?? [] }, null, 2),
  '```',
];

/** The facts and text of one ad in a prompt. */
function adOf(j: JobView): string[] {
  const d = detailOf(j);
  return [
    `Titel: ${j.title}`,
    `Unternehmen: ${j.company}`,
    `Ort: ${j.location}`,
    `Link: ${d.url}`,
    ...(j.match ? [`Passung laut App: ${j.match.score} von 100`] : []),
    '',
    d.text ?? 'Den vollständigen Anzeigentext hat die App noch nicht.',
  ];
}

/** A prompt like core's export::ai_prompt: the rubric in short, the profile, the ad. */
function promptOf(j: JobView): string {
  return [
    'Du unterstützt mich als KI-Assistent bei der Auswahl von Projekten. Bitte prüfe gründlich, wie gut diese Stellenanzeige zu meinem Beraterprofil passt.',
    '',
    ...PROMPT_PROFILE,
    '',
    'Die Anzeige',
    ...adOf(j),
  ].join('\n');
}

/**
 * Like core's export::ai_prompt_top: the best current matches (3 to 5; pinned first, then by
 * score; never excluded, hidden or gone), compared in one prompt with a ranking.
 */
function promptTopOf(limit: number): string {
  const best = jobs
    .filter((j) => j.match?.status === 'scored' && !j.hidden && j.detail.kind !== 'gone')
    .sort(
      (a, b) =>
        Number(b.pinned) - Number(a.pinned) ||
        (b.match?.score ?? 0) - (a.match?.score ?? 0) ||
        b.firstSeenAt.localeCompare(a.firstSeenAt),
    )
    .slice(0, Math.min(5, Math.max(3, limit)));
  if (best.length === 0) throw fail('notFound', { what: 'jobs' });
  return [
    'Du unterstützt mich als KI-Assistent bei der Auswahl von Projekten. Bitte vergleiche die besten aktuellen Jobs aus meiner Job-Alert-App mit meinem Beraterprofil und bring sie in eine Reihenfolge.',
    '',
    ...PROMPT_PROFILE,
    '',
    'Die Jobs',
    ...best.flatMap((j, i) => ['', `Job ${i + 1}`, ...adOf(j)]),
  ].join('\n');
}

/* --------------------------------------------------------------------- runs */

let running = false;
/** The kind of the run in progress (its end names it). */
let runningKind: RunSummary['kind'] = 'fetch';

function fail(kind: ErrorInfo['kind'], params: ErrorInfo['params'] = {}): ErrorInfo {
  return { kind, params };
}

/** Through the channel of the run, or without a run the page's channel (as Rust does). */
function emit(event: RunEvent): void {
  (runSender ?? pageSender)?.send(event);
}

const isFetch = (kind: RunSummary['kind']): boolean => kind === 'fetch' || kind === 'fullMailbox';

/** `app_state`: the page's new channel replaces the old one and takes over a running run. */
function attachPage(sender: Sender): void {
  pageSender?.release();
  pageSender = sender.hold();
  if (runSender !== null) {
    runSender.release();
    runSender = sender.hold();
  }
}

/** The run is over: its handle (and with it its channel) is dropped. */
function endRun(): void {
  running = false;
  runSender?.release();
  runSender = null;
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

/** What the export of a run reports (`?export=locked`: the Excel file is open elsewhere). */
function exported(): RunSummary['export'] {
  const written = lastRun().export!;
  if (!EXPORT_LOCKED) return written;
  return {
    ...written,
    overviewXlsx: null,
    error: {
      kind: 'fileLocked',
      params: { path: 'C:/Users/demo/Jobs/JobAlerts.xlsx', target: 'overview' },
    },
  };
}

function script(kind: RunSummary['kind']): RunEvent[] {
  const events: RunEvent[] = [
    { type: 'started', kind },
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
    ...NEW_JOBS.map((j): RunEvent => ({ type: 'jobUpdated', job: j, fresh: true })),
    { type: 'status', code: 'fetchingDetails', portal: 'linkedin', until: null },
    { type: 'progress', step: 'fetch', portal: null, done: 0, total: 2 },
    { type: 'progress', step: 'fetch', portal: null, done: 1, total: 2 },
    { type: 'progress', step: 'fetch', portal: null, done: 2, total: 2 },
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
    excludedBy('anue', 49),
  ];
  NEW_JOBS.forEach((j, i) => {
    events.push({
      type: 'jobUpdated',
      job: { ...j, detail: i === 2 ? j.detail : { kind: 'ok' }, match: results[i]! },
      fresh: true,
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
      ],
      // Three new jobs, the excluded one is none; the 88 fits well.
      newJobs: { count: 2, high: 1 },
      export: exported(),
      emptyAlerts: [],
    },
  });
  return events;
}

/** "Details holen" for jobs: their pages, their scores, no mailbox and no new jobs. */
function detailsScript(keys: JobKey[]): RunEvent[] {
  const targets = keys.map(find).filter((j): j is JobView => j !== undefined);
  const events: RunEvent[] = [
    { type: 'started', kind: 'details' },
    { type: 'status', code: 'fetchingDetails', portal: targets[0]?.portal ?? null, until: null },
    { type: 'progress', step: 'fetch', portal: null, done: 0, total: targets.length },
  ];
  targets.forEach((j, i) => {
    events.push({
      type: 'jobUpdated',
      job: { ...j, detail: { kind: 'ok' }, match: j.match ?? scored(62, ['Controlling'], 2, 3) },
      fresh: false,
    });
    events.push({
      type: 'progress',
      step: 'fetch',
      portal: null,
      done: i + 1,
      total: targets.length,
    });
  });
  events.push({ type: 'status', code: 'writingFiles', portal: null, until: null });
  events.push({
    type: 'finished',
    summary: {
      ...lastRun(),
      kind: 'details',
      startedAt: at(0.01),
      finishedAt: at(0),
      scan: null,
      newJobs: null,
      perPortal: PORTALS.filter((p) => targets.some((j) => j.portal === p)).map((portal) => ({
        portal,
        new: 0,
        known: 0,
        dup: 0,
        fetched: targets.filter((j) => j.portal === portal).length,
        failed: 0,
        gone: 0,
        skipped: 0,
        stopped: null,
      })),
      export: exported(),
      emptyAlerts: [],
    },
  });
  return events;
}

function startRun(request: RunRequest, sender: Sender | null): void {
  const kind = request.kind;
  if (running) throw fail('busy');
  if (state.mailbox.user === null && kind !== 'rescore' && kind !== 'details') {
    throw fail('mailMissing');
  }
  running = true;
  runningKind = kind;
  runSender = sender?.hold() ?? null;
  harness.done = false;
  const events =
    MAIL_OFFLINE && isFetch(kind)
      ? offlineScript()
      : request.kind === 'rescore'
        ? rescoreScript()
        : request.kind === 'details'
          ? detailsScript(request.keys)
          : script(request.kind);
  let index = 0;
  const step = (): void => {
    if (!running) return;
    if (harness.holdAfter !== null && index >= harness.holdAfter) {
      setTimeout(step, TICK);
      return;
    }
    const event = events[index++];
    if (event === undefined) return;
    apply(event);
    emit(event);
    if (event.type === 'finished') {
      endRun();
      harness.done = true;
      return;
    }
    setTimeout(step, TICK);
  };
  setTimeout(step, TICK);
}

/** The rescore the app starts after a profile change: scoring only, nothing fetched. */
function rescoreScript(): RunEvent[] {
  return [
    { type: 'started', kind: 'rescore' },
    { type: 'status', code: 'scoring', portal: null, until: null },
    { type: 'progress', step: 'score', portal: null, done: 0, total: 1 },
    { type: 'progress', step: 'score', portal: null, done: 1, total: 1 },
    {
      type: 'finished',
      summary: {
        ...lastRun(),
        kind: 'rescore',
        startedAt: at(0.01),
        finishedAt: at(0),
        scan: null,
        newJobs: null,
        perPortal: [],
        emptyAlerts: [],
        export: exported(),
      },
    },
  ];
}

function offlineScript(): RunEvent[] {
  return [
    { type: 'started', kind: 'fetch' },
    { type: 'status', code: 'connectingMail', portal: null, until: null },
    {
      type: 'finished',
      summary: {
        ...lastRun({ kind: 'failed', error: fail('mailConnect') }),
        perPortal: [],
        newJobs: { count: 0, high: 0 },
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
    // "The last fetch": a rescore or a details run never replaces it (pipeline::run).
    if (isFetch(event.summary.kind)) state.lastRun = event.summary;
    state.running = null;
  }
}

function cancelRun(): void {
  if (!running) return;
  running = false;
  const summary: RunSummary = {
    ...lastRun({ kind: 'cancelled' }),
    kind: runningKind,
    perPortal: [],
    emptyAlerts: [],
  };
  setTimeout(() => {
    const event: RunEvent = { type: 'finished', summary };
    apply(event);
    emit(event);
    endRun();
    harness.done = true;
  }, TICK);
}

/* ----------------------------------------------------------------- handlers */

type Args<K extends keyof Commands> = Omit<Commands[K]['args'], 'channel'>;
type Handlers = { [K in keyof Commands]: (args: Args<K>) => Commands[K]['result'] };

const find = (key: { portal: string; id: string }): JobView | undefined =>
  jobs.find((j) => j.key.portal === key.portal && j.key.id === key.id);

/** The Rust side of the `channel` argument of the command being handled. */
let sender: Sender | null = null;

const handlers: Handlers = {
  app_state: () => {
    if (sender !== null) attachPage(sender);
    return structuredClone(state);
  },
  start_run: ({ request }) => {
    startRun(request, sender);
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
  set_app_status: ({ key, status }) => {
    const j = find(key);
    if (j === undefined || j.appStatus === status) return false;
    j.appStatus = status;
    if (status === null) statusAt.delete(markKey(key));
    else statusAt.set(markKey(key), new Date(Date.now()).toISOString());
    refresh();
    return true;
  },
  set_note: ({ key, note }) => {
    if ([...note].length > 2000) throw fail('invalid', { reason: 'noteTooLong', max: 2000 });
    const j = find(key);
    const next = note.trim() === '' ? null : note;
    if (j === undefined || (notes.get(markKey(key)) ?? null) === next) return false;
    if (next === null) notes.delete(markKey(key));
    else notes.set(markKey(key), next);
    return true;
  },
  set_hidden: ({ key, hidden }) => {
    const j = find(key);
    if (j === undefined || j.hidden === hidden) return false;
    j.hidden = hidden;
    if (hidden) hiddenAt.set(markKey(key), new Date(Date.now()).toISOString());
    else hiddenAt.delete(markKey(key));
    refresh();
    return true;
  },
  ai_prompt: ({ key }) => {
    const j = find(key);
    if (j === undefined) throw fail('notFound', { what: 'job' });
    if (state.profile === null) throw fail('notFound', { what: 'profile' });
    return promptOf(j);
  },
  ai_prompt_top: ({ limit }) => {
    if (state.profile === null) throw fail('notFound', { what: 'profile' });
    return promptTopOf(limit);
  },
  pick_profile: () => structuredClone(FILE_DRAFT),
  parse_profile: ({ text }) => answerDraft(text),
  profile_prompt: () => PROMPT,
  save_profile: ({ save }) => {
    const after = save.after;
    if (after.focus.length > 5) throw fail('invalid', { reason: 'profileValue', field: 'focus' });
    if ((after.criteria.minDayRate ?? 0) > 100_000) {
      throw fail('invalid', { reason: 'profileValue', field: 'minDayRate' });
    }
    const form = savedForm(after);
    const count = form.competences.length + form.tools.length + form.keywords.length;
    const quality = count === 0 ? 'empty' : count < 5 ? 'thin' : 'good';
    state.profile = {
      ...PROFILE,
      fileName: 'beraterprofil.json',
      bytes: JSON.stringify(form).length,
      savedAt: at(0),
      quality,
      understood: {
        ...PROFILE.understood!,
        competenceCount: count,
        competences: form.competences.map((r) => r.name),
        warnings: [],
        focus: form.focus,
        roles: form.roles,
        wishes: form.wishes,
      },
      form,
    };
    if (jobs.every((j) => j.match === null)) {
      const sample = sampleJobs();
      for (const j of jobs) j.match = sample.find((s) => s.key.id === j.key.id)?.match ?? null;
    }
    refresh();
    return structuredClone(state.profile);
  },
  remove_profile: () => {
    state.profile = null;
    return true;
  },
  save_mailbox: ({ user, password }) => {
    if (!/^[^@\s]+@[^@\s]+\.[a-z]{2,}$/i.test(user)) {
      throw fail('invalid', { reason: 'mailAddress' });
    }
    if (!/^[a-z]{16}$/i.test(password.replace(/\s/g, ''))) {
      throw fail('invalid', { reason: 'appPassword' });
    }
    if (password.replace(/\s/g, '').toLowerCase() === WRONG_PASSWORD) throw fail('mailAuth');
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

/** Listeners of app events (`listen` of @tauri-apps/api/event). */
const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();

export async function listen<T>(
  name: string,
  handler: (event: { payload: T }) => void,
): Promise<() => void> {
  const set = listeners.get(name) ?? new Set();
  listeners.set(name, set);
  const wrapped = handler as (event: { payload: unknown }) => void;
  set.add(wrapped);
  return () => set.delete(wrapped);
}

const harness: Harness = {
  calls: [],
  emit(event) {
    apply(event);
    emit(event);
  },
  appRun(kind) {
    startRun(kind === 'details' ? { kind, keys: [] } : { kind }, null);
  },
  fire(name, payload) {
    for (const handler of listeners.get(name) ?? []) handler({ payload });
  },
  done: false,
  detailDelay: 0,
  failPages: 0,
  holdAfter: null,
  job(key) {
    const found = find(key);
    return found === undefined ? null : structuredClone(found);
  },
};
window.__harness = harness;
initial();

/* --------------------------------------------------------------------- core */

/** The app password Gmail refuses in the harness. */
const WRONG_PASSWORD = 'falschfalschfals';

/** Commands that refuse in the dry run (`ensure_real` in src-tauri): they write outside it. */
const DRY_RUN_REFUSED: ReadonlySet<string> = new Set([
  'pick_profile',
  'remove_profile',
  'save_profile_template',
  'save_mailbox',
  'remove_mailbox',
  'portal_login',
  'portal_logout',
  'rewrite_txt',
  'clear_txt',
  'reset_all',
]);

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  harness.calls.push([command, args]);
  const handler = handlers[command as keyof Commands] as ((a: unknown) => unknown) | undefined;
  if (handler === undefined) throw fail('internal', { command });
  if (state.dryRun && DRY_RUN_REFUSED.has(command)) throw fail('dryRun');
  const delay = command === 'job_detail' ? DELAY + harness.detailDelay : DELAY;
  if (delay > 0 && command !== 'report_ui_error') {
    await new Promise((resolve) => setTimeout(resolve, delay));
  }
  // Like Tauri: every call gets its own Rust-side channel, dropped when nothing holds it.
  sender = args.channel instanceof Channel ? new Sender(args.channel as Channel<RunEvent>) : null;
  const own = sender?.hold() ?? null;
  try {
    return handler(args) as T;
  } finally {
    sender = null;
    own?.release();
  }
}
