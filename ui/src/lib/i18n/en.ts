// English UI catalog: de.ts in English, key for key and in the same order. Its type is the
// German catalog's shape (`Catalog`), so a missing or extra key is a type error; after a
// merge the type check lists every new German key to translate here.
//
// The same rules as in German (CLAUDE.md, checked by core/tests/ui_contract.rs): little
// text, plain and natural. Buttons are one verb phrase without a period; notes are one short
// sentence with a period; headings and labels end without a colon; no dash or em dash as a
// separator, no "X: Y", no exclamation marks. No German except product and portal names and
// the name of the German language. Glossary: Job · Portal · Match · Details · Fetch ·
// Profile · Mailbox · Alert email · Overview · Excel file · Excluded · New · To check ·
// Favourites · Inbox (the place of the active jobs) · Archive · Trash · Skill · Preference.
// Plain British English: "email", never "mail" for one message; "preferences", never
// "wishes"; "forever" for endgültig, never "for good"; two main clauses are joined by a
// conjunction, never by a comma alone; an introductory phrase takes its comma ("Without a
// profile, …"); apostrophes and quotes are typographic (’ “ ”), a named control stands in
// quotes (“Fetch details”).

import type {
  Band,
  DetailState,
  ErrorKind,
  InvalidInput,
  JobSort,
  Language,
  PauseReason,
  Place,
  Portal,
  PortalHealth,
  LanguageLevel,
  ProfileAvailability,
  ProfileQuality,
  ReasonKind,
  ReasonWeight,
  RemoteWish,
  RunKindName,
  StatusCode,
  Step,
  VaultKind,
  WorkMode,
} from '../ipc/types';
import { textOf, type Catalog, type ContractKind, type CriterionState } from './de';
import {
  NBSP,
  formatCountdown,
  formatEuro,
  formatMoment,
  formatNumber,
  formatPercent,
} from './format';

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
  linkedin: 'linkedin.com',
  freelance: 'freelance.de',
  freelancermap: 'freelancermap.de',
};
const portalOf = (value: unknown): string =>
  typeof value === 'string' && value in portalName ? portalName[value as Portal] : str(value);
const joined = (items: string[]): string =>
  items.length < 2 ? items.join('') : `${items.slice(0, -1).join(', ')} and ${items.at(-1)}`;

/** The countries the engine can tell apart in a job ad (ISO codes of `laender`). */
const countryName: Record<string, string> = {
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
};
/** ISO codes as the engine sends them (`DE, AT`) in words: "Germany and Austria". */
const countryNames = (value: unknown): string =>
  joined(
    str(value)
      .split(',')
      .map((code) => code.trim())
      .filter((code) => code !== '')
      .map((code) => countryName[code.toUpperCase()] ?? code),
  );

const INTERNAL = 'An internal error occurred, and the log has the details.';

const errors: Record<ErrorKind | 'unknown', Text> = {
  db: 'The database reports an error.',
  fileLocked: 'A file is open in another program right now.',
  io: 'A file could not be read or written.',
  xlsx: 'The Excel file could not be written.',
  corrupt: 'The app’s data is damaged.',
  newerSchema: 'The data comes from a newer version of the app.',
  invalid: 'The input is not valid.',
  busy: 'A fetch is running already.',
  notFound: (p) =>
    p.what === 'file'
      ? 'The file does not exist.'
      : p.what === 'folder'
        ? 'The folder does not exist.'
        : 'This no longer exists.',
  dryRun: 'This does not work in the dry run.',
  mailMissing: 'No mailbox is connected.',
  mailConnect: 'Gmail cannot be reached.',
  mailAuth: 'Gmail rejected the address or the app password.',
  mailTimeout: 'Gmail is not responding.',
  mailLost: 'The connection to Gmail was lost.',
  mailNotGmail: 'This is not a Gmail mailbox.',
  mailServer: 'Gmail reports an error.',
  mailCancelled: 'Cancelled.',
  secretStore: 'The system’s password store cannot be reached.',
  secretCorrupt: 'The stored app password cannot be read.',
  portalUnavailable: (p) => `No connection to ${portalOf(p.portal)}.`,
  portalPaused: (p) => `Fetching from ${portalOf(p.portal)} is paused right now.`,
  portalQuota: (p) => `The limit for ${portalOf(p.portal)} is reached.`,
  internal: INTERNAL,
  unknown: INTERNAL,
};

/** Fields of the profile form, named in an error about their value (the label of the field
 *  without its unit, as everywhere: warnings, key names, the profile). */
const profileField: Record<string, string> = {
  name: 'Name',
  title: 'Role',
  competences: 'Skills',
  strengths: 'Key strengths',
  keywords: 'Keywords',
  years: 'Professional experience',
  degrees: 'Degrees',
  industries: 'Industries',
  tools: 'Tools and methods',
  certificates: 'Certificates',
  languages: 'Languages',
  minDayRate: 'Minimum day rate',
  countries: 'Countries',
  contracts: 'Excluded contract types',
  remoteOutside: 'Allow remote roles abroad',
  available: 'Available from',
  targetYears: 'Minimum seniority of the role',
  minSalary: 'Minimum annual salary',
  permanentPlaces: 'Locations for permanent roles',
  permanentRemoteMin: 'Minimum remote share',
  focus: 'Focus areas',
  roles: 'Target roles',
  wishDayRate: 'Preferred day rate',
  remote: 'Remote share',
  regions: 'Preferred regions',
  wishIndustries: 'Preferred industries',
};
const fieldName = (value: unknown): string => profileField[str(value)] ?? str(value);

const invalid: Record<InvalidInput['reason'], Text> = {
  noPortal: 'At least one portal must be active.',
  profileNotUtf8: 'The file is not a text file.',
  profileNotJson: (p) => `The file is not valid JSON, line ${str(p.line)}.`,
  profileNotObject: 'The file contains no profile.',
  profileValue: (p) => `The value of “${fieldName(p.field)}” is not valid.`,
  profileAnswer: 'The answer contains no profile.',
  mailAddress: 'The address is incomplete.',
  appPassword: 'An app password has 16 letters.',
  noSignIn: (p) => `There is no sign-in for ${portalOf(p.portal)}.`,
};

const status: Record<StatusCode, string> = {
  connectingMail: 'Connecting to the mailbox',
  searchingMail: 'Looking for alert emails',
  readingMails: 'Reading alert emails',
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
  throttled: 'the portal is throttling requests',
  blocked: 'the portal is blocking requests',
  layoutChanged: 'the pages look different than expected',
  stateUnreadable: 'the state of the portal cannot be read',
  network: 'the portal cannot be reached',
  challenged: 'the portal is asking for a verification',
};

/** Opening the alert email of a job in Gmail, the same words wherever it is offered. */
const OPEN_MAIL = 'Open alert email';

/** The run that reads every alert email (`fullMailbox`), one name everywhere. */
const FULL_MAILBOX = 'Read the whole mailbox';

/** What a detail state means, the same in a row's badge tooltip and in the reader. */
const detailSays = {
  teaser: 'Without a sign-in, the portal shows only the start of the ad.',
  unfetchable: 'The ad could not be fetched after several tries.',
  gone: 'The ad is no longer online.',
  onRequest: 'Older jobs get their details only on request.',
} as const;

/** Alert emails without jobs, and what to do about them (the overview and the settings). */
const emptyMails = (mails: number): string =>
  mails === 1
    ? 'One alert email had no jobs, so please check it in Gmail.'
    : `${n(mails)} alert emails had no jobs, so please check them in Gmail.`;

/** A profile file the app cannot read (the list, the overview, the Profile view). */
const PROFILE_UNREADABLE = 'Profile cannot be read';

const ANUE = 'The ad mentions temporary agency work.';
const LOW_TEXT = 'The ad has little text.';
const SHORT_TEXT = 'The ad is very short.';

/** Contract type of an ad (`contractType` params `type`, `inferred`). */
const contract: Record<ContractKind, string> = {
  interim: 'Interim',
  permanent: 'Permanent',
  anue: 'Temporary agency work',
  unclear: 'Contract type unclear',
};

function contractName(p: Params): string {
  const type = typeof p.type === 'string' && p.type in contract ? (p.type as ContractKind) : null;
  if (type === null) return contract.unclear;
  return p.inferred && type !== 'unclear'
    ? `Probably ${contract[type].toLowerCase()}`
    : contract[type];
}

/** Preferences of the profile (`state` met, near, missed or unknown). */
function dayRateWish(p: Params): string {
  const rate = formatEuro(p.rate);
  const wish = formatEuro(p.wish);
  switch (p.state) {
    case 'met':
      return `The day rate of ${rate} meets your preferred rate of ${wish}.`;
    case 'near':
      return `The day rate of ${rate} is just below your preferred rate of ${wish}.`;
    case 'missed':
      return `The day rate of ${rate} is below your preferred rate of ${wish}.`;
    default:
      return p.currency
        ? `The day rate is given in ${str(p.currency)}.`
        : 'The ad names no day rate.';
  }
}

/** The remote preference of the profile (`level` of the profile editor). */
const REMOTE_LEVEL: Record<string, string> = {
  full: 'fully remote',
  mostly: 'mostly remote',
  partly: 'partly remote',
  onSite: 'on site',
};

/** The ad's remote share next to the preference ("60% remote, and you prefer mostly remote"). */
function remoteWish(p: Params): string {
  if (p.state === 'unknown') return 'The ad names no remote share.';
  const level = typeof p.level === 'string' ? REMOTE_LEVEL[p.level] : undefined;
  const wished = level ? `, and you prefer ${level}` : '';
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
        ? 'The role is fully remote, so the region does not matter.'
        : `${str(p.location)} is in one of your preferred regions.`;
    case 'near':
      return `${str(p.location)} is outside your preferred regions, but the role is mostly remote.`;
    case 'missed':
      return `${str(p.location)} is outside your preferred regions.`;
    default:
      return 'It is unclear whether the location is in one of your preferred regions.';
  }
}

function industryWish(p: Params): string {
  switch (p.state) {
    case 'met':
      return `${str(p.wish)} is one of your preferred industries.`;
    case 'missed':
      return `${str(p.industry)} is not one of your preferred industries.`;
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
  anueRisk: 'A staffing agency gives no contract details, so temporary agency work is possible.',
  dayRate: (p) => `The day rate of ${formatEuro(p.rate)} is below ${formatEuro(p.min)}.`,
  availability: 'The availability does not fit.',
  country: (p) =>
    p.allowed
      ? `The location is outside ${countryNames(p.allowed)}.`
      : 'The location does not fit.',
  anueOptional: 'Temporary agency work is possible but not required.',
  anueHidden: 'The ad hints at temporary agency work.',
  countryUnclear: 'The location is unclear.',
  dayRateCurrency: (p) => `The rate is given in ${str(p.currency)}.`,
  availabilityGap: (p) =>
    `The start is ${count(num(p.days), 'day', 'days')} before you are available.`,
  startVague: 'The start date is unclear.',
  permanent: (p) => {
    if (p.excluded !== true) return 'This sounds like a permanent role.';
    return p.stated === true
      ? 'This is a permanent role, which the profile excludes.'
      : 'This sounds like a permanent role, which the profile excludes.';
  },
  permanentRegion: (p) =>
    p.location
      ? `The permanent role in ${str(p.location)} is outside the region in the profile.`
      : 'The permanent role is outside the region in the profile.',
  permanentRegionUnclear: (p) =>
    p.location
      ? `It is unclear whether ${str(p.location)} is in the region.`
      : 'The location of the permanent role is unclear.',
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
      ? `The role asks for ${count(num(p.years), 'year', 'years')} of experience, while the profile targets ${count(num(p.target), 'year', 'years')}.`
      : 'The role is meant for people with less experience.',
  seniorityUnclear: (p) =>
    p.junior
      ? 'The title sounds like a junior role.'
      : 'The level of experience sought is unclear.',
  overqualified: (p) =>
    p.years !== undefined && p.years !== null
      ? `The role asks for ${count(num(p.years), 'year', 'years')} of experience, and the profile has much more.`
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
      : `The ad mentions the focus area ${str(p.focus)}.`,
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
    short: 'Day rate too low',
    exclusion: 'The day rate is below the minimum in the profile.',
  },
  countries: {
    label: 'Countries',
    short: 'Outside your countries',
    exclusion: 'The location is outside the countries in the profile.',
  },
  noAnue: {
    label: 'Temporary agency work',
    short: 'Temporary agency work',
    exclusion: ANUE,
  },
  noPermanent: {
    label: 'Permanent role',
    short: 'Permanent role',
    exclusion: 'This is a permanent role, which the profile excludes.',
  },
  availability: {
    label: 'Availability',
    short: 'Start does not fit',
    exclusion: 'The start does not fit the availability.',
  },
  minSalary: {
    label: 'Annual salary',
    short: 'Salary too low',
    exclusion: 'The salary is below the minimum in the profile.',
  },
  permanentRegion: {
    label: 'Locations',
    short: 'Location outside the region',
    exclusion: 'The permanent role is outside the region in the profile.',
  },
  targetYears: {
    label: 'Experience',
    short: 'Experience does not fit',
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

/** Names of profile keys the app speaks about (the keys themselves are an external contract),
 *  named like their field in the Profile form. */
const profileKey: Record<string, string> = {
  min_tagessatz: profileField.minDayRate!,
  min_day_rate: profileField.minDayRate!,
  tagessatz_ab: profileField.minDayRate!,
  laender: profileField.countries!,
  countries: profileField.countries!,
  ausgeschlossene_vertragsarten: profileField.contracts!,
  excluded_contract_types: profileField.contracts!,
  remote_ausserhalb_erlaubt: profileField.remoteOutside!,
  remote_outside_allowed: profileField.remoteOutside!,
  verfuegbar_ab: profileField.available!,
  available_from: profileField.available!,
  min_jahresgehalt: profileField.minSalary!,
  min_annual_salary: profileField.minSalary!,
  min_salary: profileField.minSalary!,
  festanstellung_orte: profileField.permanentPlaces!,
  permanent_locations: profileField.permanentPlaces!,
  permanent_places: profileField.permanentPlaces!,
  festanstellung_remote_min: profileField.permanentRemoteMin!,
  permanent_remote_min: profileField.permanentRemoteMin!,
  zielprofil_min_jahre: profileField.targetYears!,
  target_min_years: profileField.targetYears!,
  schwerpunkte: profileField.focus!,
  focus_areas: profileField.focus!,
  wunschrollen: profileField.roles!,
  target_roles: profileField.roles!,
  tagessatz_wunsch: profileField.wishDayRate!,
  desired_day_rate: profileField.wishDayRate!,
  remote: profileField.remote!,
  regionen: profileField.regions!,
  regions: profileField.regions!,
  branchen: profileField.wishIndustries!,
  industries: profileField.wishIndustries!,
};
const keyLabel = (key: string): string => profileKey[key] ?? key;
/** Keys the engine does not read, as written in the file (so they can be found there). */
const rawKeys = (value: unknown): string[] =>
  str(value)
    .split(',')
    .map((key) => key.trim())
    .filter((key) => key !== '')
    .map((key) => `“${key}”`);

/** Profile warnings of the engine (`ProfileWarningCode`, core/src/matching/types.rs). */
const warning = {
  noCompetences: 'The profile names no skills.',
  fewCompetences: 'The profile names only a few skills.',
  noCriteria: 'The profile sets no exclusion criteria.',
  availabilityNotUnderstood: '“Available from” cannot be read.',
  ignoredKeys: (p) => `The app does not read ${joined(rawKeys(p.keys))} in the exclusion criteria.`,
  criterionNotUnderstood: (p) => `“${keyLabel(str(p.key))}” cannot be read.`,
  regionWithoutPlaces: 'The minimum remote share only works together with locations.',
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
    hidePlaces: 'Hide Archive and Trash',
    showPlaces: 'Show Archive and Trash',
    placesStay: {
      archive: 'Stays open while you are in the archive.',
      trash: 'Stays open while you are in the trash.',
    },
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
    tip: 'Resize',
    reset: 'Double-click to reset',
  },
  selection: {
    count: (value: number) => `${n(value)} selected`,
    clear: 'Clear selection',
    chosen: (value: number) => `${count(value, 'job', 'jobs')} selected`,
    commandKey: { ctrl: 'Ctrl', cmd: 'Cmd' },
    hint: (key: string) => `${key}+click adds or removes a job, Shift+click a whole range.`,
    tip: (key: string) => `Choose several jobs at once with ${key}+click.`,
  },
  place: {
    archive: 'Archive',
    trash: 'Trash',
    search: {
      inbox: 'Search jobs',
      archive: 'Search the archive',
      trash: 'Search the trash',
    } satisfies Record<Place, string>,
    count: {
      inbox: (value: number) => `${count(value, 'job', 'jobs')} in Jobs`,
      archive: (value: number) => `${count(value, 'job', 'jobs')} in the archive`,
      trash: (value: number) => `${count(value, 'job', 'jobs')} in the trash`,
    } satisfies Record<Place, (value: number) => string>,
    found: {
      inbox: (value: number, query: string) =>
        `${count(value, 'job', 'jobs')} for “${query}” in Jobs`,
      archive: (value: number, query: string) =>
        `${count(value, 'job', 'jobs')} for “${query}” in the archive`,
      trash: (value: number, query: string) =>
        `${count(value, 'job', 'jobs')} for “${query}” in the trash`,
    } satisfies Record<Place, (value: number, query: string) => string>,
    alsoIn: {
      inbox: (value: number) => `Also in Jobs (${n(value)})`,
      archive: (value: number) => `Also in the archive (${n(value)})`,
      trash: (value: number) => `Also in the trash (${n(value)})`,
    } satisfies Record<Place, (value: number) => string>,
    inArchive: 'In the archive',
    inTrash: 'In the trash',
    inTrashFor: (days: number) =>
      `In the trash, deleted forever after ${count(days, 'day', 'days')}`,
    empty: {
      inbox: 'No jobs.',
      archive: 'The archive is empty.',
      trash: 'The trash is empty.',
    } satisfies Record<Place, string>,
    reader: {
      archive: 'Archived jobs stay here until you bring them back or delete them.',
      trash: 'Deleted jobs stay here until you restore them or empty the trash.',
    } satisfies Record<Exclude<Place, 'inbox'>, string>,
    trashFor: (days: number) =>
      `Deleted jobs stay here for ${count(days, 'day', 'days')} and are then gone forever.`,
  },
  actions: {
    archive: 'Archive',
    toInbox: 'Move back to Jobs',
    trash: 'Move to trash',
    restore: 'Restore',
    purge: 'Delete forever',
    purgeHeading: (value: number) =>
      value === 1 ? 'Delete the job forever?' : `Delete ${n(value)} jobs forever?`,
    purgeText: 'Deleted jobs never come back, not even from old alert emails.',
    emptyTrash: 'Empty trash',
    emptyTrashHeading: 'Empty the trash?',
    emptyTrashText: (value: number) =>
      value === 1
        ? 'The job is deleted forever and never comes back.'
        : `The ${n(value)} jobs are deleted forever and never come back.`,
    markAllRead: 'Mark all as read',
  },
  edit: {
    undo: 'Undo',
    cut: 'Cut',
    copy: 'Copy',
    paste: 'Paste',
    delete: 'Delete',
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
    off: 'No match without a profile',
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
      open: 'Not in the profile',
      violation: 'Reason to exclude',
      check: 'To check',
    } satisfies Record<ReasonKind, string>,
    weight: {
      must: 'Required',
      nice: 'Optional',
      hard: 'Exclusion',
      info: 'Note',
    } satisfies Record<ReasonWeight, string>,
    evidenceLine: (profile: string, partial: boolean) =>
      partial ? `Partly fits “${profile}” in the profile.` : `Fits “${profile}” in the profile.`,
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
      pending: 'Details to come',
      teaser: 'Teaser only',
      failed: 'Details missing',
      unfetchable: 'Not fetchable',
      gone: 'No longer online',
      onRequest: 'Details on request',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    detailHint: {
      pending: 'The full ad has not been fetched yet.',
      teaser: detailSays.teaser,
      failed: 'The full ad could not be fetched.',
      unfetchable: detailSays.unfetchable,
      gone: detailSays.gone,
      onRequest: detailSays.onRequest,
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    closed: 'No longer taking applications',
    closedHint: 'The ad can still be read but no longer takes applications.',
    unread: 'New',
    pinned: 'Favourite',
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
    facetSaved: 'Favourites',
    sortLabel: {
      match: 'By match',
      newest: 'By date',
    } satisfies Record<JobSort, string>,
    sortNoProfile: 'Without a profile, jobs sort by date only.',
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
    statusOf: (code: StatusCode, portal: Portal | null): string => {
      const at = portal === null ? undefined : statusAt[code];
      return at !== undefined && portal !== null ? at(portalName[portal]) : status[code];
    },
    ofTotal: (total: number) => `of ${n(total)}`,
    newPill: (value: number) => `${n(value)} new`,
    topPill: (value: number) => count(value, 'fits well', 'fit well'),
    resumesIn: (ms: number) => `Resumes in ${formatCountdown(ms)}`,
    pausedWhy: (reason: PauseReason, iso: string | null) =>
      iso
        ? `Paused until ${formatMoment(iso)} because ${pause[reason]}.`
        : `Paused until the next fetch because ${pause[reason]}.`,
    quota: (iso: string) => `The limit is reached, so fetching resumes at ${formatMoment(iso)}.`,
    kind: {
      fetch: 'Fetch',
      details: 'Fetch details',
      rescore: 'Score again',
      fullMailbox: FULL_MAILBOX,
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
      overview: 'The Excel file could not be written and was left unchanged.',
      overviewLocked: 'The Excel file is open in another program and was left unchanged.',
      overviewHtml: 'The overview could not be written.',
      txt: 'Not all text files could be written.',
      txtFolder: 'The folder of the text files cannot be reached.',
      backup: 'The old Excel file could not be backed up, so the new one was not written.',
      workspace: 'The work folder cannot be reached.',
    },
    skipped: (value: number) => `${count(value, 'job is', 'jobs are')} left for the next fetch.`,
    filesFailed: (value: number) => `${count(value, 'file', 'files')} could not be written.`,
    openOverview: 'Open overview',
    history: 'History',
    collapse: 'Collapse',
    expand: 'Expand',
    alert: (portal: Portal, postings: number) =>
      `Alert email from ${portalName[portal]} with ${count(postings, 'job', 'jobs')}`,
    health: (portal: Portal, kind: Exclude<PortalHealth['kind'], 'ok'>): string => {
      const name = portalName[portal];
      switch (kind) {
        case 'paused':
          return `Paused on ${name}`;
        case 'quotaReached':
          return `Limit reached on ${name}`;
        case 'layoutSuspect':
          return `Pages on ${name} look different than expected`;
        case 'loginRequired':
          return `Sign-in needed on ${name}`;
      }
    },
    checkMailbox: 'Check mailbox',
  },
  list: {
    label: 'Jobs',
    excluded: 'Excluded',
    formalMissing: {
      degree: 'Degree missing',
      licence: 'Licence missing',
    },
    emptySources: 'One job alert per portal brings in new jobs.',
    emptyWhileRun: 'The jobs show up here as the fetch goes on.',
    createAlert: (portal: string) => `Create an alert on ${portal}`,
    readOlder: FULL_MAILBOX,
    emptyNew: 'No new jobs.',
    emptyFavourites: 'No favourites yet.',
    emptyAll: 'After the first fetch, the jobs show up here.',
    emptyAfterRun: 'The alert emails have had no jobs so far.',
    noHit: (query: string) => `No jobs for “${query}”.`,
    noHitIn: {
      new: (query: string) => `No new jobs for “${query}”.`,
      favourites: (query: string) => `No favourites for “${query}”.`,
    },
    searchAll: 'Search all',
    showAll: 'Show all',
    loadFailed: 'The list could not be loaded.',
    pageFailed: 'More jobs could not be loaded.',
    createProfile: 'Create profile',
    openProfile: 'Open profile',
    noMailbox: 'Without a mailbox, no new jobs come in.',
    noProfile: 'Without a profile, there is no match.',
    profileUnreadable: PROFILE_UNREADABLE,
    profileEmpty: 'Profile without skills',
    profileBrokenText: 'That is why the jobs show no match.',
    connectMailbox: 'Connect mailbox',
  },
  facts: {
    now: 'starts now',
    from: (date: string) => `from ${date}`,
    vague: 'Start date open',
    months: (value: number) => count(value, 'month', 'months'),
    remote: (from: number, to: number) => {
      if (from >= 100) return 'fully remote';
      if (to <= 0) return 'on site';
      return from === to
        ? `${formatPercent(from)} remote`
        : `${n(from)} to ${formatPercent(to)} remote`;
    },
    rate: (amount: number, hourly: boolean, currency: string | null, unit: boolean) => {
      const money = currency ? `${n(amount)} ${currency}` : formatEuro(amount);
      return hourly ? `${money}/hr` : unit ? `${money}/day` : money;
    },
    rateOpen: 'Rate negotiable',
    salary: (amount: number) => `${formatEuro(amount)} a year`,
    years: (value: number) => `${count(value, 'year', 'years')} of experience`,
    fullRemote: 'fully remote',
    contract,
    notMentioned: (label: string) => `${label} not mentioned`,
  },
  reader: {
    // The words of the English AI prompt ("3 of 4 must-have requirements met").
    mustMet: (met: number, total: number, partial = 0) =>
      `${n(met)} of ${n(total)} must-have requirements met` +
      (partial > 0 ? `, ${n(partial)} partly` : ''),
    noMust: 'No must-have requirements found',
    frame: 'Terms',
    anueCheck: 'It is not certain whether the role is temporary agency work.',
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
    pin: 'Mark as favourite',
    unpin: 'Remove favourite',
    archive: 'Archive',
    restore: 'Restore',
    override: 'Include anyway',
    overrideUndo: 'Exclude again',
    overridden: 'You marked this job as a match.',
    prompt: 'Copy prompt for AI assessment',
    promptShort: 'Copy prompt',
    promptHint:
      'Copies the ad and the profile as a ready prompt for an AI.',
    preliminary: 'Provisional, scored from a teaser',
    mail: OPEN_MAIL,
    noMail: 'There is no alert email for this job.',
    teaserOf: (portal: string) => `Without a sign-in, ${portal} shows only a teaser.`,
    setUpSignIn: 'Set up sign-in',
    promptNoProfile: 'Without a profile, there is nothing to assess.',
    promptNoText: 'The text of the ad is still missing.',
    mailAt: (moment: string) => `Alert email from ${moment}`,
    fetchDetails: 'Fetch details',
    why: 'Why',
    wishes: 'Preferences',
    met: 'Met',
    partial: 'Partly met',
    missing: 'Not in the profile',
    check: 'To check',
    violations: 'Excluded',
    noReasons: 'The ad names no clear requirements.',
    ad: 'Ad',
    detail: {
      pending: 'The details come with the next fetch.',
      teaser: detailSays.teaser,
      failed: 'The details could not be fetched.',
      unfetchable: detailSays.unfetchable,
      gone: detailSays.gone,
      onRequest: detailSays.onRequest,
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    closed: 'The ad no longer takes applications.',
    detailsOff: '“Fetch details” is off for this portal.',
    short: SHORT_TEXT,
    loadFailed: 'The job could not be loaded.',
  },
  overview: {
    noProfileText: 'With a profile, every job shows how well it fits.',
    profileUnreadable: PROFILE_UNREADABLE,
    label: 'Today at a glance',
    pick: 'Select a job on the left.',
    issues: 'Needs attention',
    best: 'Best new matches',
    excel: 'Open Excel file',
    promptTop: 'Copy prompt for AI comparison',
    bestInList: 'The best new jobs are at the top of the list.',
    files: 'Files',
    emptyAlerts: emptyMails,
    lastRun: 'Last fetch',
  },
  health: {
    layoutText: (mails: number) =>
      `${mails === 1 ? 'One alert email' : `${n(mails)} alert emails`} had no jobs, which may mean the email format changed.`,
    layoutPages: 'The pages of the portal look different than expected.',
    loginText: 'The sign-in has expired.',
    advice: {
      paused: (reason: PauseReason, iso: string | null) => {
        const why = pause[reason].charAt(0).toUpperCase() + pause[reason].slice(1);
        return iso
          ? `${why}, so fetching resumes by itself at ${formatMoment(iso)}.`
          : `${why}, so the next fetch tries again by itself.`;
      },
      quota: (iso: string) =>
        `The limit is reached, so fetching resumes by itself at ${formatMoment(iso)}.`,
      emptyMails,
      pages: 'The pages of the portal look different, so the next fetch tries again by itself.',
      login: 'The sign-in has expired, so please sign in again.',
    },
  },
  profile: {
    none: 'No profile yet',
    replaces: 'A new profile replaces the file.',
    create: 'Create profile',
    fromCv: 'Create from CV',
    updateFromCv: 'Update from CV',
    pick: 'Choose file',
    pickOther: 'Choose another file',
    remove: 'Remove',
    removeHeading: 'Remove profile?',
    removeText: 'The jobs then show no match. The file stays as a backup in the profile folder.',
    removed: 'Profile removed.',
    savedAt: (date: string, time: string) => `Saved ${date}, ${time}`,
    unnamed: 'Profile without a name',
    quality: {
      good: 'Complete',
      thin: 'Little content',
      empty: 'No skills',
    } satisfies Record<ProfileQuality, string>,
    qualityText: {
      good: 'The match draws on the whole profile.',
      thin: 'Few skills, so the match stays rough.',
      empty: 'Without skills, nothing is scored.',
    } satisfies Record<ProfileQuality, string>,
    rescoring: (value: number) => `${count(value, 'job is', 'jobs are')} being scored again.`,
    rescored: 'Saved, and the jobs are scored again.',
    check: 'Something to check',
    next: 'Go to the first fetch',
    understood: (terms: number) => `${count(terms, 'term', 'terms')} for the match`,
    focusCount: (focus: number) => count(focus, 'focus area', 'focus areas'),
    packs: (packs: string[]) => `specialist vocabulary for ${joined(packs)}`,
    warning,
    pack: {
      finance: 'Finance',
      sap: 'SAP',
      itProject: 'IT projects',
      hr: 'HR',
      procurement: 'Procurement',
      data: 'Data',
      pharma: 'Pharma',
      operations: 'Operations',
      sales: 'Sales',
      legal: 'Legal',
      software: 'Software',
    } as Record<string, string>,
    draft: {
      new: 'New profile',
      file: 'Profile from a file',
      answer: 'Profile from the CV',
      update: 'Profile updated from the CV',
    },
    unsaved: 'Not saved',
    review: 'Check the details and save them.',
    save: 'Save',
    discard: 'Discard',
    noChanges: 'Nothing has changed yet.',
    saved: 'Saved.',
    leaveHeading: 'Save changes?',
    leaveText: 'The changes to the profile are not saved.',
    empty: 'Still empty',
    section: {
      person: 'Person',
      competences: 'Skills and focus areas',
      experience: 'Experience and qualifications',
      wishes: 'Preferences',
      criteria: 'Exclusion criteria',
      permanent: 'Permanent roles',
      availability: 'Availability',
      understood: 'How the app reads your profile',
    },
    sectionHint: {
      wishes: 'Preferences nudge the score but never exclude a job.',
      criteria: 'A job that does not fit here counts as excluded.',
      permanent: 'These rules apply to permanent roles only.',
      availability: 'A job that starts earlier is marked to check, never excluded.',
    },
    field: {
      name: 'Name',
      title: 'Role',
      titleHint: 'The role counts for the match.',
      titlePlaceholder: 'Senior consultant',
      roles: 'Target roles',
      rolesHint: 'An ad whose title fits one scores a little higher.',
      rolesPlaceholder: 'Interim management',
      competence: 'Skill',
      competencePlaceholder: 'Project management',
      years: 'Years',
      yearsHint: 'The years count when an ad asks for years of experience.',
      aliases: 'Other terms',
      aliasesHint: 'Synonyms or German terms.',
      addCompetence: 'Add skill',
      removeCompetence: (name: string) => `Remove ${name || 'skill'}`,
      star: 'Mark as focus area',
      unstar: 'Remove focus area',
      starEmpty: 'Enter a skill first.',
      focusCount: (count: number, max: number) => `Focus areas ${count} of ${max}`,
      focusHint: 'Starred skills count twice, at most five.',
      focusFull: 'At most five focus areas.',
      focusTrimmed: (count: number) =>
        `The file names ${n(count)} focus areas, and the first five are taken.`,
      strengths: 'Key strengths',
      strengthsHint: 'They support the match but prove no requirement.',
      strengthsPlaceholder: 'Steering projects safely to their goal',
      keywords: 'Keywords',
      keywordsPlaceholder: 'Transformation, process optimisation',
      keywordsHint: 'Terms that appear in matching ads.',
      totalYears: 'Professional experience (years)',
      totalYearsHint: 'From ten years on, junior roles score low.',
      degrees: 'Degrees',
      degreesPlaceholder: 'Master',
      industries: 'Industries',
      industriesPlaceholder: 'Manufacturing',
      tools: 'Tools and methods',
      toolsPlaceholder: 'Microsoft Office',
      certificates: 'Certificates',
      certificatesPlaceholder: 'PMP',
      languages: 'Languages',
      language: 'Language',
      languagePlaceholder: 'German',
      level: 'Level',
      levelHint: 'Without a level, the app assumes B2.',
      addLanguage: 'Add language',
      removeLanguage: (name: string) => `Remove ${name || 'language'}`,
      wishRate: 'Preferred day rate (€)',
      wishRateHint: 'The minimum day rate is set in the exclusion criteria.',
      remote: 'Remote share',
      regions: 'Preferred regions',
      regionsPlaceholder: 'Munich',
      wishIndustries: 'Preferred industries',
      wishIndustriesPlaceholder: 'Energy',
      minDayRate: 'Minimum day rate (€)',
      minDayRateHint: 'A job whose rate is lower is left out.',
      countries: 'Countries',
      remoteOutside: 'Allow remote roles abroad',
      remoteOutsideHint: 'When off, the app marks fully remote roles based abroad to check.',
      remoteOutsideOff: 'Choose the countries first.',
      noAnue: 'Exclude temporary agency work',
      noPermanent: 'Exclude permanent roles',
      noPermanentHint: 'Only on clear wording, otherwise the app marks the job to check.',
      available: 'Available from',
      date: 'Date',
      datePlaceholder: '01/11/2026',
      dateInvalid: 'Enter the date as 01/11/2026.',
      targetYears: 'Minimum seniority of the role (years)',
      targetYearsHint: 'Roles for far less experienced people are left out.',
      minSalary: 'Minimum annual salary (€)',
      places: 'Locations for permanent roles',
      placesPlaceholder: 'Munich',
      remoteMin: 'Minimum remote share (%)',
      remoteMinHint:
        'Outside these locations, a permanent role counts only with at least this much remote work.',
      rounded: 'Rounded down to whole euros.',
      unreadableNumber: (value: string) => `The file said “${value}”, which is not a number.`,
      unreadableDate: (value: string) => `The file said “${value}”, which is not a date.`,
      unreadableValue: (value: string) => `The file said “${value}”, which the app cannot read.`,
      unreadableFocus: (value: string) => `“${value}” is not one of the skills.`,
      unreadableRole: (value: string) => `“${value}” names no field.`,
      removeValue: 'Remove value',
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
    levelMeaning: {
      a1: 'Beginner',
      a2: 'Elementary',
      b1: 'Intermediate',
      b2: 'Upper intermediate',
      c1: 'Fluent',
      c2: 'Proficient',
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
    country: countryName,
    reading: {
      terms: (value: number) => `${count(value, 'term counts', 'terms count')} for the match.`,
      termsLabel: 'Terms',
      more: (value: number) => `and ${n(value)} more`,
      sources: 'Read from',
      fileOnly: (name: string) => `${name}, only in the file`,
      years: 'Professional experience',
      yearsValue: (value: number) => count(value, 'year', 'years'),
      degrees: 'Degrees',
      packs: 'Specialist vocabulary',
      criteria: 'Exclusion criteria',
      none: 'None',
      from: (value: string) => `from ${value}`,
      excluded: 'excluded',
      stale: 'This is how the saved profile reads.',
      source: {
        titel: 'Role',
        kernkompetenzen: 'Skills',
        methoden_tools: 'Tools and methods',
        zertifizierungen: 'Certificates',
        branchen: 'Industries',
        sprachen: 'Languages',
        alleinstellungsmerkmale: 'Key strengths',
        keywords: 'Keywords',
        abschluss: 'Degrees',
        ausbildung: 'Degrees',
        schwerpunkte: 'Focus areas',
        stationen: 'Career stages',
        projekte: 'Projects',
      } as Record<string, string>,
    },
    paste: {
      privacy: 'The CV goes to the AI you use.',
      copied: 'The prompt is copied.',
      copyFailed: 'The prompt could not be copied.',
      copy: 'Copy prompt',
      copyAgain: 'Copy again',
      step: 'Paste it into an AI chat and attach your CV.',
      preview: 'Show prompt',
      answer: 'The AI’s answer',
      take: 'Use answer',
      takeEmpty: 'Paste the AI’s answer first.',
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
    mailRefused: 'Gmail rejected the address or app password, so enter them again with “Change”.',
    vault: {
      windowsCredentialManager: 'The app password is kept in the Windows Credential Manager.',
      macosKeychain: 'The app password is kept in the macOS keychain.',
    } satisfies Record<VaultKind, string>,
    address: 'Gmail address',
    password: 'App password',
    createPassword: 'Create app password',
    twoStep: 'An app password has 16 letters and needs 2-Step Verification.',
    addressMissing: 'The Gmail address is missing.',
    passwordMissing: 'The app password is missing.',
    twoStepAction: 'Turn on verification',
    connect: 'Connect',
    mailboxSaved: 'Mailbox connected.',
    removeMailbox: 'Remove mailbox?',
    removeMailboxText: 'The app password will be deleted, but your jobs stay.',
    autoFetch: 'Fetch on startup',
    autoFetchHint: 'When the last fetch was more than six hours ago.',
    autoArchive: 'Archive jobs after 30 days',
    autoArchiveHint: 'Favourites are never archived.',
    autoEmptyTrash: 'Empty the trash after 30 days',
    autoEmptyTrashHint: 'Deleted jobs are then gone forever.',
    active: 'Active',
    details: 'Fetch details',
    needsDetails: 'Turn on “Fetch details” first.',
    login: 'With sign-in',
    loginHint: 'Shows full ads instead of a teaser.',
    detailsOn: 'Fetches the whole ad, at a calm pace and with a daily limit.',
    detailsOff: 'Without details, jobs from this portal get no match.',
    quota: (used: number, cap: number) => `Today ${n(used)} of ${n(cap)} pages`,
    quotaHour: (used: number, cap: number) => `This hour ${n(used)} of ${n(cap)} pages`,
    /** The sign-in row of a portal: its label, and its state. */
    session: 'Sign-in',
    signedIn: 'Signed in',
    notSignedIn: 'Not signed in.',
    /** A portal that is off. */
    portalOff: 'Fetching skips this portal.',
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
    txtCount: (value: number) => `${count(value, 'ad', 'ads')} as text for an AI assessment`,
    txtLeftBehind: 'The text files are still in the old folder, and Rewrite puts them here.',
    txtNone: 'There are no text files.',
    txtRewrite: 'Rewrite',
    txtClear: 'Delete',
    txtWritten: (value: number) => `${count(value, 'file', 'files')} written.`,
    txtFailed: (value: number) => `${count(value, 'file is', 'files are')} open right now.`,
    txtCleared: (value: number) => `${count(value, 'file', 'files')} deleted.`,
    txtClearHeading: 'Delete text files?',
    txtClearText: 'Only “Rewrite” brings them back.',
    fullMailbox: FULL_MAILBOX,
    fullMailboxHint: 'Reads all alert emails, not only the new ones.',
    fullMailboxAction: 'Read mailbox',
    fullMailboxHeading: 'Read the whole mailbox?',
    fullMailboxText: 'This takes longer and fetches more pages from the portals.',
    logs: 'Logs',
    data: 'App data',
    reset: 'Reset everything',
    resetHint: 'Deletes jobs, settings, profile, app password and sign-ins.',
    resetAction: 'Reset',
    resetHeading: 'Reset everything?',
    resetText:
      'The app restarts and also deletes the Excel file, the overview and the text files in the work folder.',
    resetDone: 'The app is reset.',
    resetPartly: (value: number) =>
      `The app is reset, but ${count(value, 'item', 'items')} could not be deleted.`,
    running: 'A fetch is running right now.',
    dryRun: 'Dry run, so no data is changed.',
    language: 'Language',
    languageLabel: 'App language',
    languageHint: 'The Excel file and the overview switch at the next fetch.',
    languageName: {
      de: 'German',
      en: 'English',
    } satisfies Record<Language, string>,
  },
  firstRun: {
    // "in Gmail" stays on one line: a line never ends with the preposition.
    benefit: `The app reads your job alert emails in${NBSP}Gmail and shows which jobs fit your profile.`,
    privacy: 'Everything stays on this computer.',
    steps: 'First steps',
    mailbox: 'Mailbox',
    mailboxText: `The alert emails from ${joined(Object.values(portalName))} belong${NBSP}here.`,
    profile: 'Profile',
    profileText: 'You create the profile in the app, from your CV if you like.',
    fetch: 'First fetch',
    fetchHint: 'This takes a few minutes.',
  },
  shell: {
    loadFailed: 'The app could not load its data.',
    last: (iso: string) => `Fetched ${formatMoment(iso)}`,
    showRun: 'Show fetch',
    runFailed: (iso: string) => `Failed ${formatMoment(iso)}`,
    closing: 'The fetch is stopping, and then the app closes.',
    collapseSidebar: 'Collapse sidebar',
    expandSidebar: 'Expand sidebar',
    sidebarKey: { ctrl: 'Ctrl+B', cmd: '⌘B' },
  },
  toast: {
    rescored: 'The jobs have been scored again.',
    copied: 'Copied.',
    prompt: 'Prompt copied, ready for an AI chat.',
    archivedOne: (name: string) => `“${name}” archived.`,
    trashedOne: (name: string) => `“${name}” moved to the trash.`,
    trashedMany: (value: number) => `${n(value)} jobs moved to the trash.`,
    inboxOne: (name: string) => `“${name}” is back in Jobs.`,
    inboxMany: (value: number) => `${n(value)} jobs are back in Jobs.`,
    restoredMany: (value: number) => `${n(value)} jobs restored.`,
    allRead: 'All marked as read.',
    archivedMany: (value: number) => `${n(value)} jobs archived.`,
    restored: (name: string) => `“${name}” restored.`,
    deleted: (value: number) =>
      value === 1 ? 'The job is deleted forever.' : `${n(value)} jobs are deleted forever.`,
    trashEmptied: 'Trash emptied.',
    runDone: (value: number) =>
      value === 0
        ? 'Fetch done, nothing new.'
        : `Fetch done, ${count(value, 'new job', 'new jobs')}.`,
    runDoneFilesOld: 'Fetch done, but the files are not up to date.',
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
