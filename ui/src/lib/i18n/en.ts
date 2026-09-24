// English UI catalog: de.ts in English, key for key and in the same order. Its type is the
// German catalog's shape (`Catalog`), so a missing or extra key is a type error; after a
// merge the type check lists every new German key to translate here.
//
// The same rules as in German (CLAUDE.md, checked by core/tests/ui_contract.rs): little
// text, plain and natural. Buttons are one verb phrase without a period; notes are one short
// sentence with a period; headings and labels end without a colon; no dash or em dash as a
// separator, no "X: Y", no exclamation marks. No German except product and portal names and
// the name of the German language. Glossary: Job · Portal · Match · Details · Fetch ·
// Profile · Mailbox · Alert mail · Overview · Excel file · Excluded · New · To check ·
// Saved · Applications · Archive.

import type {
  Band,
  DetailState,
  ErrorKind,
  InvalidInput,
  JobSort,
  Language,
  PauseReason,
  Portal,
  PortalHealth,
  LanguageLevel,
  ProfileAvailability,
  ProfileQuality,
  ReasonKind,
  ReasonWeight,
  RemoteWish,
  Risk,
  RunKindName,
  StatusCode,
  Step,
  VaultKind,
  WorkMode,
} from '../ipc/types';
import { textOf, type Catalog, type ContractKind, type CriterionState } from './de';
import { formatCountdown, formatEuro, formatMoment, formatNumber, formatPercent } from './format';

type Params = Record<string, string | number | boolean | null>;
type Text = string | ((params: Params) => string);

const str = (value: unknown): string =>
  typeof value === 'string' || typeof value === 'number' ? String(value) : '';
const num = (value: unknown): number => (typeof value === 'number' ? value : Number(value) || 0);
const n = (value: number): string => formatNumber(value);
/** English plural for a count. */
const count = (value: number, one: string, many: string): string =>
  `${n(value)} ${value === 1 ? one : many}`;

const portalName: Record<Portal, string> = {
  linkedin: 'LinkedIn',
  freelance: 'freelance.de',
  freelancermap: 'freelancermap',
};
const portalOf = (value: unknown): string =>
  typeof value === 'string' && value in portalName ? portalName[value as Portal] : str(value);

const INTERNAL = 'An internal error, the log says more.';

const errors: Record<ErrorKind | 'unknown', Text> = {
  db: 'The database reports an error.',
  fileLocked: 'A file is open in another program right now.',
  io: 'A file could not be read or written.',
  xlsx: 'The Excel file could not be written.',
  corrupt: 'The data of the app is damaged.',
  newerSchema: 'The data comes from a newer version of the app.',
  invalid: 'The input is not valid.',
  busy: 'A fetch is running already.',
  notFound: 'This no longer exists.',
  dryRun: 'This does not work in the dry run.',
  mailMissing: 'No mailbox is connected.',
  mailConnect: 'Gmail cannot be reached.',
  mailAuth: 'Gmail rejects the address or the app password.',
  mailTimeout: 'Gmail does not answer.',
  mailLost: 'The connection to Gmail broke off.',
  mailNotGmail: 'This is not a Gmail mailbox.',
  mailServer: 'Gmail reports an error.',
  mailCancelled: 'Cancelled.',
  secretStore: 'The password store of the system cannot be reached.',
  secretCorrupt: 'The stored app password cannot be read.',
  portalUnavailable: (p) => `${portalOf(p.portal)} cannot be reached right now.`,
  portalPaused: (p) => `${portalOf(p.portal)} is paused right now.`,
  portalQuota: (p) => `The limit for ${portalOf(p.portal)} is reached.`,
  internal: INTERNAL,
  unknown: INTERNAL,
};

/** Fields of the profile form, named in an error about their value. */
const profileField: Record<string, string> = {
  name: 'Name',
  title: 'Role',
  competences: 'Competences',
  strengths: 'Special strengths',
  keywords: 'Keywords',
  years: 'Professional experience',
  degrees: 'Degrees',
  industries: 'Industries',
  tools: 'Tools and methods',
  certificates: 'Certificates',
  languages: 'Languages',
  minDayRate: 'Day rate from',
  countries: 'Countries',
  available: 'Available from',
  targetYears: 'Roles from this much experience',
  minSalary: 'Annual salary from',
  permanentPlaces: 'Places for permanent roles',
  permanentRemoteMin: 'Remote share from',
  focus: 'Focus areas',
  roles: 'Target roles',
  wishDayRate: 'Desired day rate',
  regions: 'Desired regions',
  wishIndustries: 'Desired industries',
};
const fieldName = (value: unknown): string => profileField[str(value)] ?? str(value);

const invalid: Record<InvalidInput['reason'], Text> = {
  noPortal: 'At least one portal must be active.',
  profileNotUtf8: 'The file is not a text file.',
  profileNotJson: (p) => `The file is not valid JSON, line ${str(p.line)}.`,
  profileNotObject: 'The file holds no profile.',
  profileValue: (p) => `The value of “${fieldName(p.field)}” does not fit.`,
  profileAnswer: 'The answer holds no profile.',
  mailAddress: 'The address is incomplete.',
  appPassword: 'An app password has 16 letters.',
  noSignIn: (p) => `${portalOf(p.portal)} offers no sign-in.`,
};

const status: Record<StatusCode, string> = {
  connectingMail: 'Connecting to the mailbox',
  searchingMail: 'Looking for alert mails',
  readingMails: 'Reading alert mails',
  fetchingDetails: 'Fetching details',
  signingIn: 'Signing in',
  waiting: 'Waiting for the portal',
  scoring: 'Scoring the jobs',
  writingFiles: 'Writing the files',
};

/** The status of a run when the backend names the portal it is about. */
const statusAt: Partial<Record<StatusCode, (portal: string) => string>> = {
  fetchingDetails: (portal) => `Fetching details from ${portal}`,
  signingIn: (portal) => `Signing in to ${portal}`,
  waiting: (portal) => `Waiting for ${portal}`,
};

/** Why a portal pauses, as the second half of one sentence (`run.pausedWhy`). */
const pause: Record<PauseReason, string> = {
  throttled: 'the portal slows the requests down',
  blocked: 'the portal blocks the requests',
  layoutChanged: 'the pages look different than expected',
  stateUnreadable: 'the state of the portal cannot be read',
  network: 'the portal cannot be reached',
  challenged: 'the portal asks for a check',
};

/** Opening the alert mail of a job in Gmail, the same words wherever it is offered. */
const OPEN_MAIL = 'Open alert mail';

const ANUE = 'The ad mentions temporary agency work.';
const LOW_TEXT = 'The ad has little text.';
const SHORT_TEXT = 'The ad is very short.';

/** Contract type of an ad (`contractType` params `type`, `inferred`). */
const contract: Record<ContractKind, string> = {
  interim: 'Interim',
  permanent: 'Permanent',
  anue: 'Agency work',
  unclear: 'Contract type unclear',
};

function contractName(p: Params): string {
  const type = typeof p.type === 'string' && p.type in contract ? (p.type as ContractKind) : null;
  if (type === null) return contract.unclear;
  return p.inferred && type !== 'unclear'
    ? `Probably ${contract[type].toLowerCase()}`
    : contract[type];
}

/** Wishes of the profile (`state` met, near, missed or unknown). */
function dayRateWish(p: Params): string {
  const rate = formatEuro(p.rate);
  const wish = formatEuro(p.wish);
  switch (p.state) {
    case 'met':
      return `The day rate of ${rate} reaches the desired ${wish}.`;
    case 'near':
      return `The day rate of ${rate} is just below the desired ${wish}.`;
    case 'missed':
      return `The day rate of ${rate} is below the desired ${wish}.`;
    default:
      return p.currency
        ? `The day rate is given in ${str(p.currency)}.`
        : 'The ad names no day rate.';
  }
}

/** The remote wish of the profile (`level` of the profile editor). */
const REMOTE_LEVEL: Record<string, string> = {
  full: 'fully remote',
  mostly: 'mostly remote',
  partly: 'partly remote',
  onSite: 'on site',
};

/** The ad's remote share next to the wish ("60% remote, the wish is mostly remote"). */
function remoteWish(p: Params): string {
  if (p.state === 'unknown') return 'The ad names no remote share.';
  const level = typeof p.level === 'string' ? REMOTE_LEVEL[p.level] : undefined;
  const wished = level ? `, the wish is ${level}` : '';
  let ad: string;
  if (p.share === 0) ad = 'The role is fully on site';
  else if (p.share === 100) ad = 'The role is fully remote';
  else if (typeof p.share === 'number') ad = `The role is ${formatPercent(p.share)} remote`;
  else if (typeof p.from === 'number' && typeof p.to === 'number')
    ad = `The role is ${str(p.from)} to ${formatPercent(p.to)} remote`;
  else ad = 'The role is partly remote';
  return `${ad}${wished}.`;
}

function regionWish(p: Params): string {
  switch (p.state) {
    case 'met':
      return p.remote === true
        ? 'The role is fully remote, the region does not matter.'
        : `${str(p.location)} is in a desired region.`;
    case 'near':
      return `${str(p.location)} is outside the desired regions, the role is mostly remote.`;
    case 'missed':
      return `${str(p.location)} is outside the desired regions.`;
    default:
      return 'It is open whether the location is in a desired region.';
  }
}

function industryWish(p: Params): string {
  switch (p.state) {
    case 'met':
      return `${str(p.wish)} is a desired industry.`;
    case 'missed':
      return `${str(p.industry)} is not one of the desired industries.`;
    default:
      return 'The ad names no industry.';
  }
}

/**
 * Reason codes of the matching engine (`Reason.code`, core/src/matching/types.rs), as in
 * de.ts. `requirement` and `term` show the ad's own words (the label).
 */
const reasonCode = {
  requirement: '',
  term: '',
  anue: ANUE,
  anueRisk: 'A staffing agency without contract details, agency work is possible.',
  dayRate: (p) => `The day rate of ${formatEuro(p.rate)} is below ${formatEuro(p.min)}.`,
  availability: 'The availability does not fit.',
  country: (p) =>
    p.allowed ? `The location is outside ${str(p.allowed)}.` : 'The location does not fit.',
  anueOptional: 'Temporary agency work is possible but not required.',
  anueHidden: 'The ad hints at temporary agency work.',
  countryUnclear: 'The location is unclear.',
  dayRateCurrency: (p) => `The rate is given in ${str(p.currency)}.`,
  availabilityGap: (p) =>
    `The start is ${count(num(p.days), 'day', 'days')} before the availability.`,
  startVague: 'The start date is open.',
  permanent: 'This sounds like a permanent role.',
  permanentRegion: (p) =>
    p.location
      ? `The permanent role in ${str(p.location)} is outside the region in the profile.`
      : 'The permanent role is outside the region in the profile.',
  permanentRegionUnclear: (p) =>
    p.location
      ? `It is unclear whether ${str(p.location)} is in the region.`
      : 'The place of the permanent role is unclear.',
  salary: (p) => {
    if (p.salary === undefined || p.salary === null || p.min === undefined) {
      return 'The salary is below the minimum in the profile.';
    }
    const amount =
      typeof p.currency === 'string' && p.currency !== 'EUR'
        ? `${n(num(p.salary))} ${p.currency}`
        : formatEuro(p.salary);
    const from = p.lowerBound ? `from ${amount}` : `of ${amount}`;
    return `The annual salary ${from} is below ${formatEuro(p.min)}.`;
  },
  salaryUnknown: 'The ad names no salary.',
  tooJunior: (p) =>
    p.years !== undefined && p.years !== null
      ? `The role asks for ${count(num(p.years), 'year', 'years')} of experience, the profile aims at ${count(num(p.target), 'year', 'years')}.`
      : 'The role is meant for people with less experience.',
  seniorityUnclear: (p) =>
    p.junior
      ? 'The title sounds like a junior role.'
      : 'The level of experience sought is unclear.',
  overqualified: (p) =>
    p.years !== undefined && p.years !== null
      ? `The role asks for ${count(num(p.years), 'year', 'years')} of experience, the profile brings much more.`
      : 'The profile has much more experience than sought.',
  contractType: (p) => contractName(p),
  formalOpen: (p) => {
    if (p.class === undefined || p.class === null) return 'The profile names no degree.';
    const what =
      p.class === 'licence'
        ? 'a licence the profile does not name'
        : 'a degree the profile does not name';
    return p.mandatory ? `The ad requires ${what}.` : `The ad prefers ${what}.`;
  },
  lowEvidence: LOW_TEXT,
  shortText: SHORT_TEXT,
  focus: (p) =>
    num(p.met) > 0 || p.inTitle === true
      ? `The focus area ${str(p.focus)} is asked for.`
      : `The ad touches the focus area ${str(p.focus)}.`,
  targetRole: (p) =>
    p.fit === 'half'
      ? `The title comes close to the target role ${str(p.role)}.`
      : `The title fits the target role ${str(p.role)}.`,
  dayRateWish,
  remoteWish,
  regionWish,
  industryWish,
} satisfies Catalog['reason']['code'];

/**
 * Hard criteria of the profile (`MatchDetail.criteria[].code`, `ProfileUnderstanding.
 * criteria[].code`), as in de.ts.
 */
const criteria = {
  minDayRate: {
    label: 'Day rate',
    exclusion: 'The day rate is below the minimum in the profile.',
  },
  countries: {
    label: 'Countries',
    exclusion: 'The location is outside the countries in the profile.',
  },
  noAnue: {
    label: 'Agency work',
    exclusion: ANUE,
  },
  availability: {
    label: 'Availability',
    exclusion: 'The start does not fit the availability.',
  },
  minSalary: {
    label: 'Annual salary',
    exclusion: 'The salary is below the minimum in the profile.',
  },
  permanentRegion: {
    label: 'Places',
    exclusion: 'The permanent role is outside the region in the profile.',
  },
  targetYears: {
    label: 'Experience',
    exclusion: 'The role asks for much less experience.',
  },
} satisfies Catalog['reader']['criterion'];

/** `JobMatch.note` / `MatchDetail.summary` codes. */
const note = {
  hardCriterion: 'An exclusion criterion applies.',
  shortText: 'Too little text for a score.',
  lowEvidence: LOW_TEXT,
  engineFailed: 'This ad could not be scored.',
} satisfies Catalog['reader']['note'];

/** Names of profile keys the app speaks about (the keys themselves are an external contract). */
const profileKey: Record<string, string> = {
  hobbys: 'Hobbies',
  referenzen: 'References',
  sprachen: 'Languages',
  zertifikate: 'Certificates',
  ausbildung: 'Education',
  min_tagessatz: 'Day rate from',
  min_day_rate: 'Day rate from',
  tagessatz_ab: 'Day rate from',
  laender: 'Countries',
  countries: 'Countries',
  ausgeschlossene_vertragsarten: 'Exclude temporary agency work',
  excluded_contract_types: 'Exclude temporary agency work',
  remote_ausserhalb_erlaubt: 'Remote outside allowed',
  remote_outside_allowed: 'Remote outside allowed',
  verfuegbar_ab: 'Available from',
  available_from: 'Available from',
  min_jahresgehalt: 'Annual salary from',
  min_annual_salary: 'Annual salary from',
  min_salary: 'Annual salary from',
  festanstellung_orte: 'Places for permanent roles',
  permanent_locations: 'Places for permanent roles',
  permanent_places: 'Places for permanent roles',
  festanstellung_remote_min: 'Remote share from',
  permanent_remote_min: 'Remote share from',
  zielprofil_min_jahre: 'Roles from this much experience',
  target_min_years: 'Roles from this much experience',
  schwerpunkte: 'Focus areas',
  focus_areas: 'Focus areas',
  wunschrollen: 'Target roles',
  target_roles: 'Target roles',
  tagessatz_wunsch: 'Desired day rate',
  desired_day_rate: 'Desired day rate',
  remote: 'Remote',
  regionen: 'Desired regions',
  regions: 'Desired regions',
  branchen: 'Industries',
  industries: 'Industries',
};
const keyLabel = (key: string): string =>
  profileKey[key] ?? key.charAt(0).toUpperCase() + key.slice(1).replace(/_/g, ' ');
const keyList = (value: unknown): string[] =>
  str(value)
    .split(',')
    .map((key) => key.trim())
    .filter((key) => key !== '')
    .map(keyLabel);
const joined = (items: string[]): string =>
  items.length < 2 ? items.join('') : `${items.slice(0, -1).join(', ')} and ${items.at(-1)}`;

/** Profile warnings of the engine (`ProfileWarningCode`, core/src/matching/types.rs). */
const warning = {
  noCompetences: 'The profile names no competences.',
  fewCompetences: 'The profile names only a few competences.',
  noCriteria: 'The profile sets no exclusion criteria.',
  availabilityNotUnderstood: '“Available from” cannot be read.',
  ignoredKeys: (p) => {
    const keys = keyList(p.keys);
    return `${joined(keys)} ${keys.length === 1 ? 'is' : 'are'} not used.`;
  },
  criterionNotUnderstood: (p) => `“${keyLabel(str(p.key))}” cannot be read.`,
  regionWithoutPlaces: 'The remote share only works together with places.',
  focusTrimmed: (p) => `Only the first ${n(num(p.max))} focus areas count.`,
} satisfies Catalog['profile']['warning'];

export const en: Catalog = {
  app: {
    name: 'Job-Alert-Monitor',
  },
  nav: {
    label: 'Sections',
    jobs: 'Jobs',
    profile: 'Profile',
    settings: 'Settings',
  },
  common: {
    loading: 'Loading',
    cancel: 'Cancel',
    save: 'Save',
    remove: 'Remove',
    change: 'Change',
    open: 'Open',
    copy: 'Copy',
    hide: 'Hide',
    back: 'Back',
    retry: 'Try again',
    undo: 'Undo',
    openFolder: 'Open folder',
    openLog: 'Open log',
  },
  portal: portalName,
  chips: {
    remove: (value: string) => `Remove ${value}`,
  },
  splitter: {
    label: 'Width of the list',
  },
  edit: {
    cut: 'Cut',
    copy: 'Copy',
    paste: 'Paste',
    selectAll: 'Select all',
  },
  field: {
    reveal: 'Show password',
    conceal: 'Hide password',
    clear: 'Clear search',
  },
  score: {
    value: (percent: string) => `Match ${percent}`,
    excluded: 'Excluded',
    provisional: 'provisional',
    unscorable: 'Not scorable',
    pending: 'Being scored',
    none: 'Not scored yet',
    band: {
      high: 'High match',
      mid: 'Medium match',
      low: 'Low match',
    } satisfies Record<Band, string>,
  },
  reason: {
    kind: {
      met: 'Met',
      partial: 'Partly met',
      open: 'Open',
      violation: 'Reason to exclude',
      check: 'To check',
    } satisfies Record<ReasonKind, string>,
    weight: {
      must: 'Must',
      nice: 'Nice to have',
      hard: 'Exclusion',
      info: 'Note',
    } satisfies Record<ReasonWeight, string>,
    evidence: (quote: string, profile: string, partial: boolean) =>
      partial
        ? `“${quote}” partly fits “${profile}” in the profile.`
        : `“${quote}” fits “${profile}” in the profile.`,
    missing: (quote: string) => `“${quote}” is not in the profile.`,
    code: reasonCode,
  },
  job: {
    workMode: {
      remote: 'Remote',
      hybrid: 'Hybrid',
      onsite: 'On site',
    } satisfies Record<WorkMode, string>,
    detail: {
      pending: 'No details',
      teaser: 'Teaser only',
      failed: 'Details missing',
      unfetchable: 'Not fetchable',
      gone: 'No longer online',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    unread: 'New',
    pinned: 'Saved',
    alsoOn: (portals: string) => `also on ${portals}`,
    untitled: 'Job without a title',
  },
  toolbar: {
    fetch: 'Fetch',
    cancel: 'Cancel',
    progress: 'Progress of the fetch',
    facet: 'View',
    facetNew: 'New',
    facetAll: 'All',
    facetPinned: 'Saved',
    sortLabel: {
      match: 'Best match',
      newest: 'Newest',
    } satisfies Record<JobSort, string>,
    search: 'Search',
    searchLabel: 'Search jobs',
    needsMailbox: 'Connect a mailbox first.',
  },
  run: {
    never: 'No fetch yet',
    step: {
      scan: 'Mailbox',
      fetch: 'Details',
      score: 'Scoring',
      export: 'Files',
    } satisfies Record<Step, string>,
    status,
    statusOf: (code: StatusCode, portal: Portal | null): string => {
      const at = portal === null ? undefined : statusAt[code];
      return at !== undefined && portal !== null ? at(portalName[portal]) : status[code];
    },
    of: (done: number, total: number) => `${n(done)} of ${n(total)}`,
    ofTotal: (total: number) => `of ${n(total)}`,
    newPill: (value: number) => `${n(value)} new`,
    topPill: (value: number) => `${n(value)} fit well`,
    resumesIn: (ms: number) => `Resumes in ${formatCountdown(ms)}`,
    pausedWhy: (reason: PauseReason, iso: string | null) =>
      iso
        ? `Paused until ${formatMoment(iso)}, ${pause[reason]}.`
        : `Paused until the next fetch, ${pause[reason]}.`,
    quota: (iso: string) => `The limit is reached, it resumes at ${formatMoment(iso)}.`,
    kind: {
      fetch: 'Fetch',
      details: 'Fetch details',
      rescore: 'Score again',
      fullMailbox: 'Whole mailbox',
    } satisfies Record<RunKindName, string>,
    done: 'Fetch done',
    rescored: 'Scored again',
    nothingNew: 'Nothing new since the last fetch.',
    cancelled: 'Fetch cancelled',
    failed: 'Fetch failed',
    details: {
      done: 'Details fetched',
      none: 'No details fetched',
      cancelled: 'Fetching details cancelled',
      failed: 'Fetching details failed',
      failedAds: (value: number) => `${count(value, 'ad', 'ads')} could not be fetched.`,
      goneAds: (value: number) => `${count(value, 'ad is', 'ads are')} no longer online.`,
    },
    rescore: {
      cancelled: 'Scoring cancelled',
      failed: 'Scoring failed',
    },
    rescoring: 'The jobs are being scored again.',
    exportFailed: {
      overview: 'The Excel file could not be written and stayed as it was.',
      overviewLocked: 'The Excel file is open in another program and stayed as it was.',
      overviewHtml: 'The overview could not be written.',
      txt: 'Not all text files could be written.',
      txtFolder: 'The folder of the text files cannot be reached.',
      backup: 'The old Excel file could not be backed up, so the new one was not written.',
    },
    skipped: (value: number) => `${count(value, 'job follows', 'jobs follow')} at the next fetch.`,
    filesFailed: (value: number) => `${count(value, 'file', 'files')} could not be written.`,
    openOverview: 'Open overview',
    history: 'History',
    collapse: 'Collapse',
    expand: 'Expand',
    alert: (portal: Portal, postings: number) =>
      `Alert mail from ${portalName[portal]} with ${count(postings, 'job', 'jobs')}`,
    health: (portal: Portal, kind: Exclude<PortalHealth['kind'], 'ok'>): string => {
      const name = portalName[portal];
      switch (kind) {
        case 'paused':
          return `${name} paused`;
        case 'quotaReached':
          return `${name} reached its limit`;
        case 'layoutSuspect':
          return `${name} looks different than expected`;
        case 'loginRequired':
          return `${name} asks for a sign-in`;
      }
    },
    checkMailbox: 'Check mailbox',
  },
  list: {
    label: 'Jobs',
    excluded: 'Excluded',
    hidden: 'Archive',
    showHidden: 'Show',
    emptySources: 'The jobs come from the alert mails of the portals.',
    createAlert: (portal: string) => `Create an alert on ${portal}`,
    readOlder: 'Read older mails',
    emptyHidden: 'The archive is empty.',
    emptyNew: 'No new jobs.',
    emptyAll: 'After the first fetch the jobs show up here.',
    emptyAfterRun: 'The alert mails held no jobs so far.',
    emptyFilter: 'There are no jobs for this right now.',
    noHit: (query: string) => `No jobs for “${query}”.`,
    showAll: 'Show all',
    loadFailed: 'The list could not be loaded.',
    pageFailed: 'More jobs could not be loaded.',
    createProfile: 'Create profile',
    openProfile: 'Open profile',
    noMailbox: 'Without a mailbox no new jobs come in.',
    connectMailbox: 'Connect mailbox',
    filter: {
      high: 'High match',
      noDetail: 'No details',
      excluded: 'Excluded',
      pinned: 'Saved',
      linkedin: `New on ${portalName.linkedin}`,
      freelancermap: `New on ${portalName.freelancermap}`,
      freelance: `New on ${portalName.freelance}`,
    },
    clearFilter: 'Remove filter',
  },
  reader: {
    mustMet: (met: number, total: number, partial = 0) =>
      `${n(met)} of ${n(total)} musts met` + (partial > 0 ? `, ${n(partial)} partly` : ''),
    noMust: 'No must requirements found',
    criteria: 'Exclusion criteria',
    contractLabel: 'Contract type',
    criterion: criteria,
    criterionState: {
      met: 'Met',
      violated: 'Violated',
      unknown: 'To check',
      unset: 'Not mentioned',
    } satisfies Record<CriterionState, string>,
    note,
    open: 'Open ad',
    close: 'Close',
    pin: 'Save job',
    hide: 'Archive',
    unhide: 'Restore',
    prompt: 'Copy as prompt',
    preliminary: 'Provisional, scored from a teaser',
    mail: OPEN_MAIL,
    fetchDetails: 'Fetch details',
    why: 'Why',
    met: 'Met',
    partial: 'Partly met',
    missing: 'Open',
    check: 'To check',
    violations: 'Excluded',
    noReasons: 'The ad names no clear requirements.',
    ad: 'Ad',
    detail: {
      pending: 'The details follow at the next fetch.',
      teaser: 'Without a sign-in the portal shows only a teaser.',
      failed: 'The details could not be fetched.',
      unfetchable: 'Details cannot be fetched from this portal.',
      gone: 'The ad is no longer online.',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    detailsOff: 'Fetch details is off for this portal.',
    short: SHORT_TEXT,
    loadFailed: 'The job could not be loaded.',
  },
  overview: {
    label: 'Today at a glance',
    issues: 'Open points',
    best: 'Best match',
    excel: 'Open Excel file',
    emptyAlerts: (value: number) =>
      value === 1 ? 'One alert mail held no jobs.' : `${n(value)} alert mails held no jobs.`,
    lastRun: 'Last fetch',
    noProfile: 'No profile yet',
    noProfileText: 'With a profile every job shows how well it fits.',
    profileUnreadable: 'Profile cannot be read',
    profileEmpty: 'Profile without competences',
    profileBrokenText: 'So the jobs show no match.',
  },
  health: {
    layoutText: (mails: number) =>
      `${mails === 1 ? 'One alert mail' : `${n(mails)} alert mails`} held no jobs, maybe the mail format changed.`,
    layoutPages: 'The pages of the portal look different than expected.',
    loginText: 'The sign-in has expired.',
    advice: {
      paused: (reason: PauseReason, iso: string | null) => {
        const why = pause[reason].charAt(0).toUpperCase() + pause[reason].slice(1);
        return iso
          ? `${why}, the fetch goes on by itself at ${formatMoment(iso)}.`
          : `${why}, the next fetch tries again by itself.`;
      },
      quota: (iso: string) =>
        `The limit is reached, the fetch goes on by itself at ${formatMoment(iso)}.`,
      emptyMails: (mails: number) =>
        `${mails === 1 ? 'One alert mail' : `${n(mails)} alert mails`} held no jobs, please check in Gmail whether there are any.`,
      pages: 'The pages of the portal look different, the next fetch tries again by itself.',
      login: 'The sign-in has expired, please sign in again.',
    },
  },
  profile: {
    none: 'No profile yet',
    replaces: 'A new profile replaces the file.',
    create: 'Create profile',
    fromCv: 'Create from CV',
    pick: 'Choose file',
    pickOther: 'Choose another file',
    remove: 'Remove',
    removeHeading: 'Remove profile?',
    removeText: 'Without a profile the jobs show no match.',
    savedAt: (date: string, time: string) => `Saved ${date}, ${time}`,
    unnamed: 'Profile without a name',
    quality: {
      good: 'Complete',
      thin: 'Little content',
      empty: 'No competences',
    } satisfies Record<ProfileQuality, string>,
    qualityText: {
      good: 'The match draws on the whole profile.',
      thin: 'Few competences, so the match stays rough.',
      empty: 'Without competences nothing is scored.',
    } satisfies Record<ProfileQuality, string>,
    rescoring: (value: number) => `${count(value, 'job is', 'jobs are')} being scored again.`,
    rescored: 'Saved, jobs scored again.',
    check: 'Please check',
    next: 'Go to the first fetch',
    understood: (terms: number) => `${count(terms, 'term', 'terms')} for the match`,
    focusCount: (focus: number) => count(focus, 'focus area', 'focus areas'),
    packs: (packs: string[]) => `Domains ${packs.join(', ')}`,
    warning,
    pack: {
      finance: 'Finance',
      sap: 'SAP',
      itProject: 'IT projects',
    } as Record<string, string>,
    draft: {
      new: 'New profile',
      file: 'Profile from a file',
      answer: 'Profile from the CV',
    },
    unsaved: 'Not saved yet',
    review: 'Check the details, then save.',
    save: 'Save',
    discard: 'Discard',
    saved: 'Saved.',
    unsavedShort: 'Not saved',
    leaveHeading: 'Save changes?',
    leaveText: 'The changes to the profile are not saved.',
    empty: 'Still empty',
    section: {
      person: 'Person',
      competences: 'Competences and focus areas',
      experience: 'Experience',
      tools: 'Tools and certificates',
      languages: 'Languages',
      wishes: 'Wishes',
      criteria: 'Exclusion criteria',
      permanent: 'Permanent roles',
    },
    sectionHint: {
      wishes: 'Wishes shift the score a little, they exclude nothing.',
      criteria: 'A job that does not fit here counts as excluded.',
    },
    field: {
      name: 'Name',
      title: 'Role',
      titlePlaceholder: 'Project manager',
      roles: 'Target roles',
      rolesPlaceholder: 'Team lead',
      competence: 'Competence',
      competencePlaceholder: 'Project management',
      years: 'Years',
      aliases: 'Also called',
      addCompetence: 'Add competence',
      removeCompetence: (name: string) => `Remove ${name || 'competence'}`,
      star: 'Mark as focus area',
      focusCount: (count: number, max: number) => `Focus areas ${count} of ${max}`,
      focusHint: 'Mark up to five competences with the star.',
      focusFull: 'At most five focus areas.',
      strengths: 'Special strengths',
      strengthsPlaceholder: 'Delivered large projects on time',
      keywords: 'Keywords',
      keywordsPlaceholder: 'Agile, change management',
      keywordsHint: 'Terms that appear in matching ads.',
      totalYears: 'Professional experience (years)',
      degrees: 'Degrees',
      degreesPlaceholder: 'Master of Science',
      industries: 'Industries',
      industriesPlaceholder: 'Mechanical engineering',
      tools: 'Tools and methods',
      toolsPlaceholder: 'Microsoft Excel',
      certificates: 'Certificates',
      certificatesPlaceholder: 'PMP',
      language: 'Language',
      languagePlaceholder: 'German',
      level: 'Level',
      addLanguage: 'Add language',
      removeLanguage: (name: string) => `Remove ${name || 'language'}`,
      wishRate: 'Desired day rate (€)',
      remote: 'Work location',
      regions: 'Desired regions',
      regionsPlaceholder: 'Munich',
      wishIndustries: 'Desired industries',
      wishIndustriesPlaceholder: 'Chemicals',
      minDayRate: 'Day rate from (€)',
      countries: 'Countries',
      remoteOutside: 'Allow remote roles abroad',
      noAnue: 'Exclude temporary agency work',
      available: 'Available from',
      date: 'Date',
      datePlaceholder: '01/11/2026',
      dateInvalid: 'Enter the date as 01/11/2026.',
      targetYears: 'Roles from this much experience (years)',
      targetYearsHint: 'Roles for much less experience drop out.',
      minSalary: 'Annual salary from (€)',
      places: 'Places for permanent roles',
      placesPlaceholder: 'Munich',
      remoteMin: 'Remote share from (%)',
      remoteMinHint: 'Elsewhere a role counts only from this remote share.',
      unreadableNumber: (value: string) => `The file said “${value}”, which is not a number.`,
      unreadableDate: (value: string) => `The file said “${value}”, which is not a date.`,
      unreadablePlaces: (value: string) => `The file said “${value}”, which are not places.`,
    },
    level: {
      a1: 'A1',
      a2: 'A2',
      b1: 'B1',
      b2: 'B2',
      c1: 'C1',
      c2: 'C2',
      native: 'Native',
    } satisfies Record<LanguageLevel, string>,
    remoteWish: {
      full: 'Fully remote',
      mostly: 'Mostly remote',
      partly: 'Partly remote',
      onSite: 'On site',
    } satisfies Record<RemoteWish, string>,
    availability: {
      now: 'Now',
      from: 'From a date',
    } satisfies Record<Exclude<ProfileAvailability['kind'], 'unset'>, string>,
    country: {
      DE: 'Germany',
      AT: 'Austria',
      CH: 'Switzerland',
      NL: 'Netherlands',
      FR: 'France',
      IT: 'Italy',
      ES: 'Spain',
      PL: 'Poland',
      CZ: 'Czechia',
      GB: 'United Kingdom',
      US: 'USA',
      IN: 'India',
    } as Record<string, string>,
    paste: {
      copied: 'The prompt is copied.',
      copyFailed: 'The prompt could not be copied.',
      copy: 'Copy prompt',
      copyAgain: 'Copy again',
      step: 'Paste it into an AI and attach the CV.',
      answer: "Paste the AI's answer",
      take: 'Use answer',
    },
  },
  settings: {
    mailbox: 'Mailbox',
    fetch: 'Fetch',
    portals: 'Portals',
    files: 'Files',
    maintenance: 'Maintenance',
    connected: 'Connected',
    notConnected: 'No mailbox connected.',
    /** The last fetch could not reach Gmail, or Gmail refused the password. */
    unreachable: 'Not reachable',
    refused: 'Refused',
    mailRefused: 'Gmail refuses the address or app password, enter them again with Change.',
    vault: {
      windowsCredentialManager: 'The app password is kept in the Windows Credential Manager.',
      macosKeychain: 'The app password is kept in the macOS keychain.',
    } satisfies Record<VaultKind, string>,
    address: 'Gmail address',
    password: 'App password',
    passwordHint: '16 letters, created in the Google account.',
    createPassword: 'Create app password',
    twoStep: 'An app password needs 2-Step Verification.',
    addressMissing: 'The Gmail address is missing.',
    passwordMissing: 'The app password is missing.',
    twoStepAction: 'Turn on verification',
    connect: 'Connect',
    removeMailbox: 'Remove mailbox?',
    removeMailboxText: 'The app password is deleted, the jobs stay.',
    autoFetch: 'Fetch at start',
    autoFetchHint: 'When the last fetch is more than six hours ago.',
    autoArchive: 'Archive jobs after 30 days',
    autoArchiveHint: 'Favourites are never archived.',
    autoEmptyTrash: 'Empty the trash after 30 days',
    autoEmptyTrashHint: 'Deleted jobs are then gone for good.',
    active: 'Active',
    details: 'Fetch details',
    needsDetails: 'Turn on Fetch details first.',
    login: 'With sign-in',
    loginHint: 'Shows whole ads instead of a teaser.',
    risk: {
      low: 'Low risk',
      grey: 'Grey area',
      account: 'Account at risk',
    } satisfies Record<Risk, string>,
    riskText: {
      low: 'Only public pages from your own alert mails.',
      grey: 'Guest access, no account is affected.',
      account: 'Signed in, your own account is at stake.',
    } satisfies Record<Risk, string>,
    riskInfo: {
      low: 'The app opens only what anyone can see in a browser.',
      grey: 'The portal does not expressly allow automated reading.',
      account: 'At worst the portal locks your own account.',
    } satisfies Record<Risk, string>,
    detailsOff: 'Without details the jobs of this portal get no match.',
    quota: (used: number, cap: number) => `Today ${n(used)} of ${n(cap)} pages`,
    quotaHour: (used: number, cap: number) => `This hour ${n(used)} of ${n(cap)} pages`,
    /** The sign-in row of a portal: its label, and its state. */
    session: 'Sign-in',
    signedIn: 'Signed in',
    notSignedIn: 'Not signed in.',
    /** A portal that is off. */
    portalOff: 'The fetch skips it.',
    sessionLeft: 'The sign-in is still stored.',
    signIn: 'Sign in',
    signOut: 'Sign out',
    openPortal: 'Open in browser',
    signInWaiting: 'The sign-in window is open.',
    workspace: 'Work folder',
    workspaceDefault: 'Default',
    excel: 'Excel file',
    excelMissing: 'The Excel file is created at the first fetch.',
    txt: 'Text files',
    txtCount: (value: number) => count(value, 'file', 'files'),
    txtNone: 'There are no text files yet.',
    txtRewrite: 'Write again',
    txtClear: 'Delete',
    txtWritten: (value: number) => `${count(value, 'file', 'files')} written.`,
    txtFailed: (value: number) => `${count(value, 'file is', 'files are')} open right now.`,
    txtCleared: (value: number) => `${count(value, 'file', 'files')} deleted.`,
    txtClearHeading: 'Delete text files?',
    txtClearText: 'The next fetch creates them again.',
    fullMailbox: 'Read the whole mailbox',
    fullMailboxHint: 'Reads all alert mails, not only the new ones.',
    fullMailboxAction: 'Read mailbox',
    fullMailboxHeading: 'Read the whole mailbox?',
    fullMailboxText: 'This takes longer and fetches more pages from the portals.',
    logs: 'Logs',
    data: 'App data',
    reset: 'Reset everything',
    resetHint: 'Deletes jobs, settings, profile and app password.',
    resetAction: 'Reset',
    resetHeading: 'Reset everything?',
    resetText: 'The app restarts and is empty afterwards.',
    resetDone: 'The app is reset.',
    resetPartly: (value: number) =>
      `The app is reset, ${count(value, 'file', 'files')} could not be deleted.`,
    running: 'A fetch is running right now.',
    dryRun: 'Dry run, no data is changed.',
    language: 'Language',
    languageLabel: 'App language',
    languageHint: 'The Excel file and the overview follow at the next fetch.',
    languageName: {
      de: 'Deutsch',
      en: 'English',
    } satisfies Record<Language, string>,
  },
  firstRun: {
    benefit: 'The app reads the alert mails from Gmail and shows which jobs fit the profile.',
    privacy: 'Everything stays on this computer.',
    steps: 'First steps',
    mailbox: 'Mailbox',
    mailboxText: 'The alert mails of the portals must go to this Gmail address.',
    profile: 'Profile',
    profileText: 'The profile is made in the app, from the CV if you like.',
    fetch: 'First fetch',
    fetchHint: 'This takes a few minutes.',
  },
  shell: {
    loadFailed: 'The app could not load its data.',
    last: (iso: string) => `Fetched ${formatMoment(iso)}`,
    showRun: 'Show fetch',
    runFailed: (iso: string) => `Failed ${formatMoment(iso)}`,
    closing: 'The fetch is stopping, then the app closes.',
  },
  toast: {
    saved: 'Saved.',
    mailboxSaved: 'Mailbox connected.',
    rescored: 'The jobs are scored again.',
    copied: 'Copied.',
    prompt: 'Prompt copied, ready for an AI chat.',
    hidden: 'Archived.',
    runDone: (value: number) =>
      value === 0
        ? 'Fetch done, nothing new.'
        : `Fetch done, ${count(value, 'new job', 'new jobs')}.`,
    runDoneFilesOld: 'Fetch done, the files are not up to date.',
  },
  error: {
    text: (kind: ErrorKind | 'unknown', params: Params): string => {
      if (kind === 'invalid' && typeof params.reason === 'string' && params.reason in invalid) {
        return textOf(invalid[params.reason as InvalidInput['reason']], params);
      }
      return textOf(errors[kind], params);
    },
  },
};
