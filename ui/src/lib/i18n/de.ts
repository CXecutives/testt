// German UI catalog - the only source of UI text.
//
// Style rules (CLAUDE.md, checked by core/tests/ui_contract.rs): little text, plain and
// human. Buttons are one verb phrase without a period; notes are one short sentence with a
// period; headings and labels end without a colon; no dash or em dash as a separator, no
// "X: Y", no exclamation marks, no text twice. Glossary: Job · Portal · Passung · Details ·
// Abrufen · Profil · Postfach · Alert-Mail · Übersicht · Excel-Datei · Ausgeschlossen · Neu ·
// Zu prüfen · Favorit (Favoriten) · Bewerbung · Archiv. A profile field has one name: the
// label of its form field (without the unit) in errors, warnings and the profile.
//
// Every code of the generated types has exactly one text here: the tables are typed as
// `Record<Code, ...>`, so a new code without a text is a type error.

import type {
  AppStatus,
  Band,
  DetailState,
  ErrorKind,
  InvalidInput,
  JobSort,
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
import { formatCountdown, formatEuro, formatMoment, formatNumber, formatPercent } from './format';

type Params = Record<string, string | number | boolean | null>;
type Text = string | ((params: Params) => string);

const str = (value: unknown): string =>
  typeof value === 'string' || typeof value === 'number' ? String(value) : '';
const num = (value: unknown): number => (typeof value === 'number' ? value : Number(value) || 0);
const n = (value: number): string => formatNumber(value);
/** German plural for a count. */
const count = (value: number, one: string, many: string): string =>
  `${n(value)} ${value === 1 ? one : many}`;

const portalName: Record<Portal, string> = {
  linkedin: 'LinkedIn',
  freelance: 'freelance.de',
  freelancermap: 'freelancermap',
};
const portalOf = (value: unknown): string =>
  typeof value === 'string' && value in portalName ? portalName[value as Portal] : str(value);

const INTERNAL = 'Ein interner Fehler, mehr steht im Protokoll.';

const errors: Record<ErrorKind | 'unknown', Text> = {
  db: 'Die Datenbank meldet einen Fehler.',
  fileLocked: 'Eine Datei ist gerade in einem anderen Programm geöffnet.',
  io: 'Eine Datei ließ sich nicht lesen oder schreiben.',
  xlsx: 'Die Excel-Datei ließ sich nicht schreiben.',
  corrupt: 'Die Daten der App sind beschädigt.',
  newerSchema: 'Die Daten stammen von einer neueren Version der App.',
  invalid: 'Die Eingabe passt nicht.',
  busy: 'Gerade läuft schon ein Abruf.',
  notFound: 'Das gibt es nicht mehr.',
  dryRun: 'Im Probelauf geht das nicht.',
  mailMissing: 'Es ist kein Postfach verbunden.',
  mailConnect: 'Gmail ist nicht erreichbar.',
  mailAuth: 'Gmail lehnt Adresse oder App-Passwort ab.',
  mailTimeout: 'Gmail antwortet nicht.',
  mailLost: 'Die Verbindung zu Gmail ist abgebrochen.',
  mailNotGmail: 'Das ist kein Gmail-Postfach.',
  mailServer: 'Gmail meldet einen Fehler.',
  mailCancelled: 'Abgebrochen.',
  secretStore: 'Der Passwortspeicher des Systems ist nicht erreichbar.',
  secretCorrupt: 'Das gespeicherte App-Passwort ist nicht lesbar.',
  portalUnavailable: (p) => `${portalOf(p.portal)} ist gerade nicht erreichbar.`,
  portalPaused: (p) => `${portalOf(p.portal)} pausiert gerade.`,
  portalQuota: (p) => `Das Limit für ${portalOf(p.portal)} ist erreicht.`,
  internal: INTERNAL,
  unknown: INTERNAL,
};

/** Fields of the profile form, named in an error about their value. */
const profileField: Record<string, string> = {
  name: 'Name',
  title: 'Rolle',
  competences: 'Kompetenzen',
  strengths: 'Besondere Stärken',
  keywords: 'Stichworte',
  years: 'Berufserfahrung',
  degrees: 'Abschlüsse',
  industries: 'Branchen',
  tools: 'Werkzeuge und Methoden',
  certificates: 'Zertifikate',
  languages: 'Sprachen',
  minDayRate: 'Tagessatz ab',
  countries: 'Einsatzländer',
  available: 'Verfügbar ab',
  targetYears: 'Verlangte Erfahrung ab',
  minSalary: 'Jahresgehalt ab',
  permanentPlaces: 'Orte',
  permanentRemoteMin: 'Remote-Anteil ab',
  focus: 'Schwerpunkte',
  roles: 'Wunschrollen',
  wishDayRate: 'Wunschtagessatz',
  regions: 'Wunschregionen',
  wishIndustries: 'Wunschbranchen',
};
const fieldName = (value: unknown): string => profileField[str(value)] ?? str(value);

const invalid: Record<InvalidInput['reason'], Text> = {
  noPortal: 'Mindestens ein Portal muss aktiv sein.',
  profileNotUtf8: 'Die Datei ist keine Textdatei.',
  profileNotJson: (p) => `Die Datei ist kein gültiges JSON, Zeile ${str(p.line)}.`,
  profileNotObject: 'Die Datei enthält kein Profil.',
  profileValue: (p) => `Der Wert bei „${fieldName(p.field)}“ passt nicht.`,
  profileAnswer: 'In der Antwort steht kein Profil.',
  mailAddress: 'Die Adresse ist unvollständig.',
  appPassword: 'Ein App-Passwort hat 16 Buchstaben.',
  noSignIn: (p) => `${portalOf(p.portal)} bietet keine Anmeldung.`,
  noteTooLong: (p) => `Die Notiz ist länger als ${n(num(p.max))} Zeichen.`,
};

const status: Record<StatusCode, string> = {
  connectingMail: 'Verbindet mit dem Postfach',
  searchingMail: 'Sucht Alert-Mails',
  readingMails: 'Liest Alert-Mails',
  fetchingDetails: 'Holt Details',
  signingIn: 'Meldet sich an',
  waiting: 'Wartet auf das Portal',
  scoring: 'Bewertet die Jobs',
  writingFiles: 'Schreibt die Dateien',
};

/** The status of a run when the backend names the portal it is about. */
const statusAt: Partial<Record<StatusCode, (portal: string) => string>> = {
  fetchingDetails: (portal) => `Holt Details von ${portal}`,
  signingIn: (portal) => `Meldet sich bei ${portal} an`,
  waiting: (portal) => `Wartet auf ${portal}`,
};

/** Why a portal pauses, as the second half of one sentence (`run.pausedWhy`). */
const pause: Record<PauseReason, string> = {
  throttled: 'das Portal bremst die Anfragen',
  blocked: 'das Portal blockiert die Anfragen',
  layoutChanged: 'die Seiten sehen anders aus als erwartet',
  stateUnreadable: 'der Stand des Portals ist nicht lesbar',
  network: 'das Portal ist nicht erreichbar',
  challenged: 'das Portal verlangt eine Prüfung',
};

/** Opening the alert mail of a job in Gmail, the same words wherever it is offered. */
const OPEN_MAIL = 'Alert-Mail öffnen';

const ANUE = 'Die Anzeige nennt Arbeitnehmerüberlassung.';
const LOW_TEXT = 'Die Anzeige hat wenig Text.';
const SHORT_TEXT = 'Die Anzeige ist sehr kurz.';

/** Contract type of an ad (`contractType` params `type`, `inferred`). */
const contract = {
  interim: 'Interim',
  permanent: 'Festanstellung',
  anue: 'Arbeitnehmerüberlassung',
  unclear: 'Vertragsart unklar',
} as const;
export type ContractKind = keyof typeof contract;

function contractName(p: Params): string {
  const type = typeof p.type === 'string' && p.type in contract ? (p.type as ContractKind) : null;
  if (type === null) return contract.unclear;
  return p.inferred && type !== 'unclear' ? `Vermutlich ${contract[type]}` : contract[type];
}

/** Wishes of the profile (`state` met, near, missed or unknown). */
function dayRateWish(p: Params): string {
  const rate = formatEuro(p.rate);
  const wish = formatEuro(p.wish);
  switch (p.state) {
    case 'met':
      return `Der Tagessatz von ${rate} erreicht den Wunsch von ${wish}.`;
    case 'near':
      return `Der Tagessatz von ${rate} liegt knapp unter dem Wunsch von ${wish}.`;
    case 'missed':
      return `Der Tagessatz von ${rate} liegt unter dem Wunsch von ${wish}.`;
    default:
      return p.currency
        ? `Der Tagessatz ist in ${str(p.currency)} angegeben.`
        : 'Die Anzeige nennt keinen Tagessatz.';
  }
}

/** The remote wish of the profile (`level` of the profile editor). */
const REMOTE_LEVEL: Record<string, string> = {
  full: 'voll remote',
  mostly: 'überwiegend remote',
  partly: 'teilweise remote',
  onSite: 'vor Ort',
};

/** The ad's remote share next to the wish ("zu 60 % remote, gewünscht ist überwiegend remote"). */
function remoteWish(p: Params): string {
  if (p.state === 'unknown') return 'Die Anzeige nennt keinen Remote-Anteil.';
  const level = typeof p.level === 'string' ? REMOTE_LEVEL[p.level] : undefined;
  const wished = level ? `, gewünscht ist ${level}` : '';
  let ad: string;
  if (p.share === 0) ad = 'Die Stelle ist ganz vor Ort';
  else if (p.share === 100) ad = 'Die Stelle ist ganz remote';
  else if (typeof p.share === 'number') ad = `Die Stelle ist zu ${formatPercent(p.share)} remote`;
  else if (typeof p.from === 'number' && typeof p.to === 'number')
    ad = `Die Stelle ist zu ${str(p.from)} bis ${formatPercent(p.to)} remote`;
  else ad = 'Die Stelle ist teilweise remote';
  return `${ad}${wished}.`;
}

function regionWish(p: Params): string {
  switch (p.state) {
    case 'met':
      return p.remote === true
        ? 'Die Stelle ist voll remote, die Region spielt keine Rolle.'
        : `${str(p.location)} liegt in einer Wunschregion.`;
    case 'near':
      return `${str(p.location)} liegt außerhalb der Wunschregionen, die Stelle ist überwiegend remote.`;
    case 'missed':
      return `${str(p.location)} liegt außerhalb der Wunschregionen.`;
    default:
      return 'Ob der Einsatzort in einer Wunschregion liegt, steht nicht fest.';
  }
}

function industryWish(p: Params): string {
  switch (p.state) {
    case 'met':
      return `Die Branche ${str(p.wish)} ist gewünscht.`;
    case 'missed':
      return `${str(p.industry)} gehört nicht zu den Wunschbranchen.`;
    default:
      return 'Die Anzeige nennt keine Branche.';
  }
}

/**
 * Reason codes of the matching engine (`Reason.code`, core/src/matching/types.rs). One entry
 * per code: a new engine code needs exactly one line here. `requirement` and `term` show the
 * ad's own words (the label).
 */
const reasonCode = {
  requirement: '',
  term: '',
  anue: ANUE,
  anueRisk: 'Ein Personaldienstleister ohne Angaben zum Vertrag, Überlassung ist möglich.',
  dayRate: (p) => `Der Tagessatz von ${formatEuro(p.rate)} liegt unter ${formatEuro(p.min)}.`,
  availability: 'Die Verfügbarkeit passt nicht.',
  country: (p) =>
    p.allowed
      ? `Der Einsatzort liegt außerhalb von ${str(p.allowed)}.`
      : 'Der Einsatzort passt nicht.',
  anueOptional: 'Arbeitnehmerüberlassung ist möglich, aber nicht Pflicht.',
  anueHidden: 'Die Anzeige deutet auf Arbeitnehmerüberlassung hin.',
  countryUnclear: 'Der Einsatzort ist unklar.',
  dayRateCurrency: (p) => `Der Satz ist in ${str(p.currency)} angegeben.`,
  availabilityGap: (p) =>
    `Der Start liegt ${count(num(p.days), 'Tag', 'Tage')} vor der Verfügbarkeit.`,
  startVague: 'Die Anzeige nennt keinen Starttermin.',
  permanent: 'Das klingt nach einer Festanstellung.',
  permanentRegion: (p) =>
    p.location
      ? `Die Festanstellung in ${str(p.location)} liegt außerhalb der Region im Profil.`
      : 'Die Festanstellung liegt außerhalb der Region im Profil.',
  permanentRegionUnclear: (p) =>
    p.location
      ? `Ob ${str(p.location)} in der Region liegt, ist unklar.`
      : 'Der Arbeitsort der Festanstellung ist unklar.',
  salary: (p) => {
    if (p.salary === undefined || p.salary === null || p.min === undefined) {
      return 'Das Gehalt liegt unter dem Minimum im Profil.';
    }
    const amount =
      typeof p.currency === 'string' && p.currency !== 'EUR'
        ? `${n(num(p.salary))} ${p.currency}`
        : formatEuro(p.salary);
    const from = p.lowerBound ? `ab ${amount}` : `von ${amount}`;
    return `Das Jahresgehalt ${from} liegt unter ${formatEuro(p.min)}.`;
  },
  salaryUnknown: 'Die Anzeige nennt kein Gehalt.',
  tooJunior: (p) =>
    p.years !== undefined && p.years !== null
      ? `Die Stelle verlangt ${count(num(p.years), 'Jahr', 'Jahre')} Erfahrung, das Profil zielt auf ${count(num(p.target), 'Jahr', 'Jahre')}.`
      : 'Die Stelle richtet sich an weniger Erfahrene.',
  seniorityUnclear: (p) =>
    p.junior
      ? 'Der Titel klingt nach einer Einstiegsstelle.'
      : 'Das gesuchte Erfahrungslevel ist unklar.',
  overqualified: (p) =>
    p.years !== undefined && p.years !== null
      ? `Gesucht ${num(p.years) === 1 ? 'ist' : 'sind'} ${count(num(p.years), 'Jahr', 'Jahre')} Erfahrung, das Profil bringt deutlich mehr mit.`
      : 'Das Profil ist deutlich erfahrener als gesucht.',
  contractType: (p) => contractName(p),
  formalOpen: (p) => {
    if (p.class === undefined || p.class === null) return 'Das Profil nennt keinen Abschluss.';
    const what =
      p.class === 'licence'
        ? 'eine Zulassung, die das Profil nicht nennt'
        : 'einen Abschluss, den das Profil nicht nennt';
    return p.mandatory ? `Die Anzeige verlangt ${what}.` : `Die Anzeige wünscht ${what}.`;
  },
  lowEvidence: LOW_TEXT,
  shortText: SHORT_TEXT,
  focus: (p) =>
    num(p.met) > 0 || p.inTitle === true
      ? `Gefragt ist der Schwerpunkt ${str(p.focus)}.`
      : `Die Anzeige streift den Schwerpunkt ${str(p.focus)}.`,
  targetRole: (p) =>
    p.fit === 'half'
      ? `Der Titel kommt der Wunschrolle ${str(p.role)} nahe.`
      : `Der Titel passt zur Wunschrolle ${str(p.role)}.`,
  dayRateWish,
  remoteWish,
  regionWish,
  industryWish,
} satisfies Record<string, Text>;
export type ReasonCode = keyof typeof reasonCode;

interface CriterionText {
  /** Short name in the criteria strip of the reader. */
  label: string;
  /** Why a job is excluded by it. */
  exclusion: string;
}

/**
 * Hard criteria of the profile (`MatchDetail.criteria[].code`, `ProfileUnderstanding.
 * criteria[].code`). One entry per criterion: a new criterion needs exactly one entry here.
 */
const criteria = {
  minDayRate: {
    label: 'Tagessatz',
    exclusion: 'Der Tagessatz liegt unter dem Minimum im Profil.',
  },
  countries: {
    label: 'Einsatzländer',
    exclusion: 'Der Einsatzort liegt außerhalb der Länder im Profil.',
  },
  noAnue: {
    label: 'Arbeitnehmerüberlassung',
    exclusion: ANUE,
  },
  availability: {
    label: 'Verfügbarkeit',
    exclusion: 'Der Start passt nicht zur Verfügbarkeit.',
  },
  minSalary: {
    label: 'Jahresgehalt',
    exclusion: 'Das Gehalt liegt unter dem Minimum im Profil.',
  },
  permanentRegion: {
    label: 'Orte',
    exclusion: 'Die Festanstellung liegt außerhalb der Region im Profil.',
  },
  targetYears: {
    label: 'Erfahrung',
    exclusion: 'Die Stelle verlangt deutlich weniger Erfahrung.',
  },
} satisfies Record<string, CriterionText>;
export type CriterionKey = keyof typeof criteria;

export type CriterionState = 'met' | 'violated' | 'unknown' | 'unset';

/** `JobMatch.note` / `MatchDetail.summary` codes. */
const note = {
  hardCriterion: 'Ein Ausschlusskriterium greift.',
  shortText: 'Zu wenig Text für eine Bewertung.',
  lowEvidence: LOW_TEXT,
  engineFailed: 'Diese Anzeige ließ sich nicht bewerten.',
} satisfies Record<string, Text>;
export type MatchNote = keyof typeof note;

/** Names of profile keys the app speaks about (the keys themselves are an external contract). */
const profileKey: Record<string, string> = {
  // Keys the engine does not read (`ignoredKeys`).
  hobbys: 'Hobbys',
  referenzen: 'Referenzen',
  sprachen: 'Sprachen',
  zertifikate: 'Zertifikate',
  ausbildung: 'Ausbildung',
  // The criteria keys (German and English), named like their field in the Profil
  // form (the label without its unit).
  min_tagessatz: 'Tagessatz ab',
  min_day_rate: 'Tagessatz ab',
  tagessatz_ab: 'Tagessatz ab',
  laender: 'Einsatzländer',
  countries: 'Einsatzländer',
  ausgeschlossene_vertragsarten: 'Arbeitnehmerüberlassung ausschließen',
  excluded_contract_types: 'Arbeitnehmerüberlassung ausschließen',
  remote_ausserhalb_erlaubt: 'Remote außerhalb erlaubt',
  remote_outside_allowed: 'Remote außerhalb erlaubt',
  verfuegbar_ab: 'Verfügbar ab',
  available_from: 'Verfügbar ab',
  min_jahresgehalt: 'Jahresgehalt ab',
  min_annual_salary: 'Jahresgehalt ab',
  min_salary: 'Jahresgehalt ab',
  festanstellung_orte: 'Orte',
  permanent_locations: 'Orte',
  permanent_places: 'Orte',
  festanstellung_remote_min: 'Remote-Anteil ab',
  permanent_remote_min: 'Remote-Anteil ab',
  zielprofil_min_jahre: 'Verlangte Erfahrung ab',
  target_min_years: 'Verlangte Erfahrung ab',
  // Schwerpunkte, target roles and wishes (German and English keys).
  schwerpunkte: 'Schwerpunkte',
  focus_areas: 'Schwerpunkte',
  wunschrollen: 'Wunschrollen',
  target_roles: 'Wunschrollen',
  tagessatz_wunsch: 'Wunschtagessatz',
  desired_day_rate: 'Wunschtagessatz',
  remote: 'Remote',
  regionen: 'Wunschregionen',
  regions: 'Wunschregionen',
  branchen: 'Branchen',
  industries: 'Branchen',
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
  items.length < 2 ? items.join('') : `${items.slice(0, -1).join(', ')} und ${items.at(-1)}`;

/** Profile warnings of the engine (`ProfileWarningCode`, core/src/matching/types.rs). */
const warning = {
  noCompetences: 'Das Profil nennt keine Kompetenzen.',
  fewCompetences: 'Das Profil nennt nur wenige Kompetenzen.',
  noCriteria: 'Das Profil setzt keine Ausschlusskriterien.',
  availabilityNotUnderstood: '„Verfügbar ab“ ist nicht lesbar.',
  // Keys of a criteria section the engine does not read (a typo, an unknown rule).
  ignoredKeys: (p) => {
    const keys = keyList(p.keys);
    return `${joined(keys)} ${keys.length === 1 ? 'bleibt' : 'bleiben'} unberücksichtigt.`;
  },
  criterionNotUnderstood: (p) => `„${keyLabel(str(p.key))}“ ist nicht lesbar.`,
  regionWithoutPlaces: 'Der Remote-Anteil wirkt nur zusammen mit Orten.',
  focusTrimmed: (p) => `Nur die ersten ${n(num(p.max))} Schwerpunkte zählen.`,
} satisfies Record<string, Text>;
export type ProfileWarning = keyof typeof warning;

export const de = {
  app: {
    name: 'Job-Alert-Monitor',
  },
  nav: {
    label: 'Bereiche',
    jobs: 'Jobs',
    profile: 'Profil',
    settings: 'Einstellungen',
  },
  common: {
    loading: 'Wird geladen',
    cancel: 'Abbrechen',
    save: 'Speichern',
    remove: 'Entfernen',
    change: 'Ändern',
    open: 'Öffnen',
    copy: 'Kopieren',
    hide: 'Ausblenden',
    back: 'Zurück',
    retry: 'Erneut versuchen',
    undo: 'Rückgängig',
    openFolder: 'Ordner öffnen',
    openLog: 'Protokoll öffnen',
  },
  portal: portalName,
  chips: {
    remove: (value: string) => `${value} entfernen`,
  },
  splitter: {
    label: 'Breite der Liste',
  },
  /** The native context menu of fields and selected text (the OS's words). */
  edit: {
    cut: 'Ausschneiden',
    copy: 'Kopieren',
    paste: 'Einfügen',
    selectAll: 'Alles auswählen',
  },
  field: {
    reveal: 'Passwort zeigen',
    conceal: 'Passwort verbergen',
    clear: 'Suche leeren',
  },
  score: {
    /** `Passung 87 %` - the number comes formatted from format.ts. */
    value: (percent: string) => `Passung ${percent}`,
    excluded: 'Ausgeschlossen',
    /** Aria label part of a ring whose score comes from a teaser only. */
    provisional: 'vorläufig',
    unscorable: 'Nicht bewertbar',
    pending: 'Wird bewertet',
    none: 'Noch nicht bewertet',
    /** A ring without a usable profile. */
    off: 'Ohne Profil keine Passung',
    band: {
      high: 'Hohe Passung',
      mid: 'Mittlere Passung',
      low: 'Geringe Passung',
    } satisfies Record<Band, string>,
  },
  reason: {
    kind: {
      met: 'Erfüllt',
      partial: 'Teilweise erfüllt',
      open: 'Nicht im Profil',
      violation: 'Ausschlussgrund',
      check: 'Zu prüfen',
    } satisfies Record<ReasonKind, string>,
    weight: {
      must: 'Pflicht',
      nice: 'Optional',
      hard: 'Ausschluss',
      info: 'Hinweis',
    } satisfies Record<ReasonWeight, string>,
    /** The line under a reason: only the profile's side (the ad's words stand above it). */
    evidenceLine: (profile: string, partial: boolean) =>
      partial ? `Teilweise durch „${profile}“ im Profil.` : `Passt zu „${profile}“ im Profil.`,
    /** Tooltip of a reason: the ad's words and what the profile says. */
    evidence: (quote: string, profile: string, partial: boolean) =>
      partial
        ? `„${quote}“ passt teilweise zu „${profile}“ im Profil.`
        : `„${quote}“ passt zu „${profile}“ im Profil.`,
    missing: (quote: string) => `„${quote}“ steht nicht im Profil.`,
    code: reasonCode,
  },
  job: {
    workMode: {
      remote: 'Remote',
      hybrid: 'Hybrid',
      onsite: 'Vor Ort',
    } satisfies Record<WorkMode, string>,
    /** Badge per DetailState kind (`ok` shows none). */
    detail: {
      pending: 'Ohne Details',
      teaser: 'Nur Anriss',
      failed: 'Details nicht geholt',
      unfetchable: 'Nicht abrufbar',
      gone: 'Nicht mehr online',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    /** What a detail badge means, in its tooltip. */
    detailHint: {
      pending: 'Die ganze Anzeige ist noch nicht geholt.',
      teaser: 'Das Portal zeigt ohne Anmeldung nur einen Anriss.',
      failed: 'Die ganze Anzeige ließ sich nicht holen.',
      unfetchable: 'Von diesem Portal lassen sich keine Details holen.',
      gone: 'Die Anzeige ist nicht mehr online.',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    unread: 'Neu',
    pinned: 'Favorit',
    alsoOn: (portals: string) => `auch auf ${portals}`,
    untitled: 'Job ohne Titel',
  },
  toolbar: {
    fetch: 'Abrufen',
    cancel: 'Abbrechen',
    progress: 'Fortschritt des Abrufs',
    facet: 'Auswahl',
    facetNew: 'Neu',
    facetAll: 'Alle',
    facetSaved: 'Favoriten',
    /** The order of the list in words (the sort button). */
    sortLabel: {
      match: 'Beste Passung',
      newest: 'Neueste',
    } satisfies Record<JobSort, string>,
    search: 'Suchen',
    searchLabel: 'Jobs durchsuchen',
    needsMailbox: 'Erst ein Postfach verbinden.',
  },
  run: {
    never: 'Noch kein Abruf',
    step: {
      scan: 'Postfach',
      fetch: 'Details',
      score: 'Bewertung',
      export: 'Dateien',
    } satisfies Record<Step, string>,
    status,
    /** The status, naming the portal where the backend says which one. */
    statusOf: (code: StatusCode, portal: Portal | null): string => {
      const at = portal === null ? undefined : statusAt[code];
      return at !== undefined && portal !== null ? at(portalName[portal]) : status[code];
    },
    of: (done: number, total: number) => `${n(done)} von ${n(total)}`,
    /** After the rolling number of a step counter: "von 7". */
    ofTotal: (total: number) => `von ${n(total)}`,
    newPill: (value: number) => `${n(value)} neu`,
    topPill: (value: number) => count(value, 'passt gut', 'passen gut'),
    resumesIn: (ms: number) => `Weiter in ${formatCountdown(ms)}`,
    /** A paused portal in one sentence: until when, then why. */
    pausedWhy: (reason: PauseReason, iso: string | null) =>
      iso
        ? `Pause bis ${formatMoment(iso)}, ${pause[reason]}.`
        : `Pause bis zum nächsten Abruf, ${pause[reason]}.`,
    quota: (iso: string) => `Das Limit ist erreicht, weiter ab ${formatMoment(iso)}.`,
    kind: {
      fetch: 'Abruf',
      details: 'Details holen',
      rescore: 'Neu bewerten',
      fullMailbox: 'Ganzes Postfach',
    } satisfies Record<RunKindName, string>,
    done: 'Abruf fertig',
    rescored: 'Neu bewertet',
    nothingNew: 'Nichts Neues seit dem letzten Abruf.',
    cancelled: 'Abruf abgebrochen',
    failed: 'Abruf fehlgeschlagen',
    /** A details run (the reader's "Details holen"): its title, what it did not get. */
    details: {
      done: 'Details geholt',
      none: 'Keine Details geholt',
      cancelled: 'Details holen abgebrochen',
      failed: 'Details holen fehlgeschlagen',
      failedAds: (value: number) =>
        `${count(value, 'Anzeige ließ', 'Anzeigen ließen')} sich nicht holen.`,
      goneAds: (value: number) =>
        `${count(value, 'Anzeige ist', 'Anzeigen sind')} nicht mehr online.`,
    },
    /** A rescore the card speaks about (only when something went wrong). */
    rescore: {
      cancelled: 'Bewertung abgebrochen',
      failed: 'Bewertung fehlgeschlagen',
    },
    rescoring: 'Die Jobs werden gerade neu bewertet.',
    /**
     * A file the export could not write (`export.error.params.target`); the old file stays.
     * `overviewLocked`: the Excel file is open in another program.
     */
    exportFailed: {
      overview: 'Die Excel-Datei ließ sich nicht schreiben und blieb unverändert.',
      overviewLocked:
        'Die Excel-Datei ist in einem anderen Programm geöffnet und blieb unverändert.',
      overviewHtml: 'Die Übersicht ließ sich nicht schreiben.',
      txt: 'Nicht alle Textdateien ließen sich schreiben.',
      txtFolder: 'Der Ordner der Textdateien ist nicht erreichbar.',
      backup: 'Die alte Excel-Datei ließ sich nicht sichern, die neue wurde nicht geschrieben.',
    },
    skipped: (value: number) => `${count(value, 'Job folgt', 'Jobs folgen')} beim nächsten Abruf.`,
    filesFailed: (value: number) =>
      count(value, 'Datei ließ', 'Dateien ließen') + ' sich nicht schreiben.',
    openOverview: 'Übersicht öffnen',
    history: 'Verlauf',
    collapse: 'Einklappen',
    expand: 'Ausklappen',
    alert: (portal: Portal, postings: number) =>
      `Alert-Mail von ${portalName[portal]} mit ${count(postings, 'Job', 'Jobs')}`,
    /** A line of the history when a portal's health changes. */
    health: (portal: Portal, kind: Exclude<PortalHealth['kind'], 'ok'>): string => {
      const name = portalName[portal];
      switch (kind) {
        case 'paused':
          return `${name} pausiert`;
        case 'quotaReached':
          return `${name} hat das Limit erreicht`;
        case 'layoutSuspect':
          return `${name} sieht anders aus als erwartet`;
        case 'loginRequired':
          return `${name} verlangt eine Anmeldung`;
      }
    },
    checkMailbox: 'Postfach prüfen',
  },
  list: {
    label: 'Jobs',
    /** The divider (its count is a pill of its own, left out where the rows are a part). */
    excluded: 'Ausgeschlossen',
    archive: 'Archiv',
    showArchive: 'Anzeigen',
    archiveLink: (value: number) => `Archiv ${n(value)}`,
    /** Search hits among the archived jobs, under the live ones. */
    inArchive: 'Im Archiv',
    emptyArchive: 'Archiv leeren',
    emptyArchiveHeading: 'Archiv leeren?',
    emptyArchiveText: 'Die Jobs werden gelöscht und kommen nicht wieder.',
    /** The empty list says where jobs come from and how to get more. */
    emptySources: 'Die Jobs kommen aus den Alert-Mails der Portale.',
    createAlert: (portal: string) => `Alert auf ${portal} anlegen`,
    readOlder: 'Ältere Mails lesen',
    emptySent: 'Noch keine Bewerbung vermerkt.',
    emptyArchived: 'Das Archiv ist leer.',
    emptyNew: 'Keine neuen Jobs.',
    emptyAll: 'Nach dem ersten Abruf stehen die Jobs hier.',
    emptyAfterRun: 'Die Alert-Mails enthielten bisher keine Jobs.',
    emptyFilter: 'Dazu gibt es gerade keine Jobs.',
    noHit: (query: string) => `Keine Jobs zu „${query}“.`,
    showAll: 'Alle zeigen',
    loadFailed: 'Die Liste ließ sich nicht laden.',
    pageFailed: 'Weitere Jobs ließen sich nicht laden.',
    createProfile: 'Profil anlegen',
    openProfile: 'Profil öffnen',
    noMailbox: 'Ohne Postfach kommen keine neuen Jobs dazu.',
    /** No usable profile: said once, at the top of the list. */
    noProfile: 'Ohne Profil gibt es keine Passung.',
    profileUnreadable: 'Profil nicht lesbar',
    profileEmpty: 'Profil ohne Kompetenzen',
    profileBrokenText: 'Die Jobs zeigen deshalb keine Passung.',
    connectMailbox: 'Postfach verbinden',
    filter: {
      high: 'Hohe Passung',
      noDetail: 'Ohne Details',
      excluded: 'Ausgeschlossen',
      pinned: 'Favoriten',
      linkedin: `Neu auf ${portalName.linkedin}`,
      freelancermap: `Neu auf ${portalName.freelancermap}`,
      freelance: `Neu auf ${portalName.freelance}`,
    },
    clearFilter: 'Filter entfernen',
  },
  /** The key facts of an ad in short words (list row, criteria chips). */
  facts: {
    now: 'ab sofort',
    from: (date: string) => `ab ${date}`,
    vague: 'Start offen',
    months: (value: number) => count(value, 'Monat', 'Monate'),
    remote: (from: number, to: number) => {
      if (from >= 100) return 'voll remote';
      if (to <= 0) return 'vor Ort';
      return from === to
        ? `${formatPercent(from)} remote`
        : `${n(from)} bis ${formatPercent(to)} remote`;
    },
    /** `1.100 €`, with the unit `1.100 €/Tag`, per hour `95 €/Std.`, `1.000 CHF/Tag`. */
    rate: (amount: number, hourly: boolean, currency: string | null, unit: boolean) => {
      const money = currency ? `${n(amount)} ${currency}` : formatEuro(amount);
      return hourly ? `${money}/Std.` : unit ? `${money}/Tag` : money;
    },
    rateOpen: 'Satz nach Absprache',
    salary: (amount: number) => `${formatEuro(amount)} im Jahr`,
    years: (value: number) => `${count(value, 'Jahr', 'Jahre')} Erfahrung`,
    fullRemote: 'Voll remote',
    contract,
    /** A criterion the ad does not mention. */
    notMentioned: (label: string) => `${label} nicht genannt`,
  },
  reader: {
    mustMet: (met: number, total: number, partial = 0) =>
      `${n(met)} von ${n(total)} Pflichtanforderungen erfüllt` +
      (partial > 0 ? `, ${n(partial)} teilweise` : ''),
    noMust: 'Keine Pflichtanforderungen erkannt',
    criteria: 'Ausschlusskriterien',
    /** The label of the strip of hard criteria next to the score. */
    frame: 'Rahmen',
    /** Why the temporary agency criterion needs a look. */
    anueCheck: 'Ob die Stelle über Arbeitnehmerüberlassung läuft, steht nicht fest.',
    contractLabel: 'Vertragsart',
    criterion: criteria,
    criterionState: {
      met: 'Erfüllt',
      violated: 'Verletzt',
      unknown: 'Zu prüfen',
      unset: 'Nicht genannt',
    } satisfies Record<CriterionState, string>,
    note,
    open: 'Anzeige öffnen',
    close: 'Schließen',
    pin: 'Als Favorit markieren',
    unpin: 'Favorit entfernen',
    archive: 'Archivieren',
    restore: 'Wiederherstellen',
    archived: 'Dieser Job ist archiviert.',
    deleteForGood: 'Endgültig löschen',
    deleteHeading: 'Job endgültig löschen?',
    deleteText: 'Der Job wird gelöscht und kommt auch mit alten Alert-Mails nicht wieder.',
    /** An excluded job the user counts anyway, and back. */
    override: 'Trotzdem passend',
    overrideUndo: 'Wieder ausschließen',
    overridden: 'Von dir als passend markiert.',
    prompt: 'Prompt für KI-Bewertung kopieren',
    promptShort: 'KI-Bewertung',
    promptHint:
      'Kopiert Anzeige und Profil als fertigen Prompt für ChatGPT, Claude oder eine andere KI.',
    /** Under the band of a score that comes from a teaser only. */
    preliminary: 'Vorläufig, aus einem Anriss bewertet',
    appStatus: {
      saved: 'Favorit',
      sent: 'Beworben',
    } satisfies Record<AppStatus, string>,
    mail: OPEN_MAIL,
    fetchDetails: 'Details holen',
    why: 'Warum',
    wishes: 'Wünsche',
    met: 'Erfüllt',
    partial: 'Teilweise erfüllt',
    missing: 'Nicht im Profil',
    check: 'Zu prüfen',
    violations: 'Ausgeschlossen',
    noReasons: 'Die Anzeige nennt keine klaren Anforderungen.',
    ad: 'Anzeige',
    detail: {
      pending: 'Die Details folgen beim nächsten Abruf.',
      teaser: 'Ohne Anmeldung zeigt das Portal nur einen Anriss.',
      failed: 'Die Details ließen sich nicht holen.',
      unfetchable: 'Von diesem Portal lassen sich keine Details holen.',
      gone: 'Die Anzeige ist nicht mehr online.',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    detailsOff: 'Details holen ist für dieses Portal aus.',
    short: SHORT_TEXT,
    loadFailed: 'Der Job ließ sich nicht laden.',
  },
  overview: {
    label: 'Tagesüberblick',
    issues: 'Offene Punkte',
    best: 'Neu und passend',
    excel: 'Excel öffnen',
    /** The best matches as one prompt for any AI chat. */
    promptTop: 'Prompt für KI-Vergleich kopieren',
    promptTopNone: 'Noch kein Job bewertet.',
    /** Under the portal's name, so the sentence does not name it again. */
    emptyAlerts: (value: number) =>
      value === 1
        ? 'Eine Alert-Mail enthielt keine Jobs.'
        : `${n(value)} Alert-Mails enthielten keine Jobs.`,
    lastRun: 'Letzter Abruf',
  },
  health: {
    layoutText: (mails: number) =>
      `${mails === 1 ? 'Eine Alert-Mail enthielt' : `${n(mails)} Alert-Mails enthielten`} keine Jobs, vielleicht hat sich das Mail-Format geändert.`,
    layoutPages: 'Die Seiten des Portals sehen anders aus als erwartet.',
    loginText: 'Die Anmeldung ist abgelaufen.',
    /** A portal problem in the settings, in one sentence that says whether to act. */
    advice: {
      paused: (reason: PauseReason, iso: string | null) => {
        const why = pause[reason].charAt(0).toUpperCase() + pause[reason].slice(1);
        return iso
          ? `${why}, der Abruf macht ab ${formatMoment(iso)} von selbst weiter.`
          : `${why}, der nächste Abruf versucht es von selbst wieder.`;
      },
      quota: (iso: string) =>
        `Das Limit ist erreicht, der Abruf macht ab ${formatMoment(iso)} von selbst weiter.`,
      emptyMails: (mails: number) =>
        `${mails === 1 ? 'Eine Alert-Mail enthielt' : `${n(mails)} Alert-Mails enthielten`} keine Jobs, bitte in Gmail nachsehen, ob dort welche stehen.`,
      pages:
        'Die Seiten des Portals sehen anders aus, der nächste Abruf versucht es von selbst wieder.',
      login: 'Die Anmeldung ist abgelaufen, bitte neu anmelden.',
    },
  },
  profile: {
    none: 'Noch kein Profil',
    noneText: 'Gegen das Profil wird jeder Job geprüft.',
    create: 'Profil anlegen',
    fromCv: 'Aus Lebenslauf erstellen',
    pick: 'Datei wählen',
    remove: 'Entfernen',
    removeHeading: 'Profil entfernen?',
    removeText: 'Ohne Profil zeigen die Jobs keine Passung mehr.',
    meta: (size: string, date: string) => (date ? `${size} · ${date}` : size),
    quality: {
      good: 'Gut lesbar',
      thin: 'Wenig Inhalt',
      empty: 'Ohne Kompetenzen',
    } satisfies Record<ProfileQuality, string>,
    qualityText: {
      good: 'Die Passung stützt sich auf das ganze Profil.',
      thin: 'Wenige Kompetenzen, die Passung bleibt grob.',
      empty: 'Ohne Kompetenzen wird nichts bewertet.',
    } satisfies Record<ProfileQuality, string>,
    rescoring: (value: number) => `${count(value, 'Job wird', 'Jobs werden')} neu bewertet.`,
    rescored: 'Neu bewertet.',
    /** The badge of a well filled profile that still has something to check. */
    check: 'Bitte prüfen',
    next: 'Weiter zum ersten Abruf',
    parseError: 'Das Profil ist nicht mehr lesbar.',
    understood: (competences: number) =>
      `${count(competences, 'Kompetenz', 'Kompetenzen')} erkannt`,
    focusCount: (focus: number) => count(focus, 'Schwerpunkt', 'Schwerpunkte'),
    packs: (packs: string[]) => `Fachgebiete ${packs.join(', ')}`,
    warning,
    pack: {
      finance: 'Finanzen',
      sap: 'SAP',
      itProject: 'IT-Projekte',
    } as Record<string, string>,
    draft: {
      new: 'Neues Profil',
      file: 'Profil aus einer Datei',
      answer: 'Profil aus dem Lebenslauf',
    },
    unsaved: 'Noch nicht gespeichert',
    review: 'Die Angaben prüfen, dann speichern.',
    save: 'Speichern',
    discard: 'Verwerfen',
    saved: 'Das Profil ist gespeichert.',
    leaveHeading: 'Änderungen verwerfen?',
    leaveText: 'Das Profil hat Änderungen, die noch nicht gespeichert sind.',
    empty: 'Noch leer',
    section: {
      person: 'Person',
      competences: 'Kompetenzen und Schwerpunkte',
      experience: 'Erfahrung',
      tools: 'Werkzeuge und Zertifikate',
      languages: 'Sprachen',
      wishes: 'Wünsche',
      criteria: 'Ausschlusskriterien',
      permanent: 'Festanstellung',
    },
    sectionHint: {
      wishes: 'Wünsche verschieben die Bewertung leicht, sie schließen nichts aus.',
      criteria: 'Ein Job, der hier nicht passt, gilt als ausgeschlossen.',
    },
    field: {
      name: 'Name',
      title: 'Rolle',
      titlePlaceholder: 'Projektleitung',
      roles: 'Wunschrollen',
      rolesPlaceholder: 'Teamleitung',
      competence: 'Kompetenz',
      competencePlaceholder: 'Projektmanagement',
      years: 'Jahre',
      aliases: 'Auch genannt',
      aliasesPlaceholder: 'Project Management',
      addCompetence: 'Kompetenz hinzufügen',
      removeCompetence: (name: string) => `${name || 'Kompetenz'} entfernen`,
      star: 'Als Schwerpunkt markieren',
      focus: 'Schwerpunkte',
      focusHint: 'Mit dem Stern bis zu fünf Kompetenzen markieren.',
      focusFull: 'Höchstens fünf Schwerpunkte.',
      strengths: 'Besondere Stärken',
      strengthsPlaceholder: 'Große Projekte im Zeitplan übergeben',
      keywords: 'Stichworte',
      keywordsPlaceholder: 'Agil, Change Management',
      keywordsHint: 'Begriffe, die in passenden Anzeigen stehen.',
      totalYears: 'Berufserfahrung (Jahre)',
      degrees: 'Abschlüsse',
      degreesPlaceholder: 'Master of Science',
      industries: 'Branchen',
      industriesPlaceholder: 'Maschinenbau',
      tools: 'Werkzeuge und Methoden',
      toolsPlaceholder: 'Microsoft Excel',
      certificates: 'Zertifikate',
      certificatesPlaceholder: 'PMP',
      language: 'Sprache',
      languagePlaceholder: 'Englisch',
      level: 'Niveau',
      addLanguage: 'Sprache hinzufügen',
      removeLanguage: (name: string) => `${name || 'Sprache'} entfernen`,
      wishRate: 'Wunschtagessatz (€)',
      remote: 'Remote',
      regions: 'Wunschregionen',
      regionsPlaceholder: 'München',
      wishIndustries: 'Wunschbranchen',
      wishIndustriesPlaceholder: 'Chemie',
      minDayRate: 'Tagessatz ab (€)',
      countries: 'Einsatzländer',
      remoteOutside: 'Remote außerhalb erlaubt',
      remoteOutsideHint: 'Dann zählen auch Stellen im Ausland, die ganz remote sind.',
      noAnue: 'Arbeitnehmerüberlassung ausschließen',
      available: 'Verfügbar ab',
      date: 'Datum',
      datePlaceholder: '01.11.2026',
      dateInvalid: 'Datum im Format 01.11.2026 eingeben.',
      targetYears: 'Verlangte Erfahrung ab (Jahre)',
      targetYearsHint: 'Stellen für deutlich weniger Erfahrung fallen weg.',
      minSalary: 'Jahresgehalt ab (€)',
      places: 'Orte',
      placesPlaceholder: 'München',
      remoteMin: 'Remote-Anteil ab (%)',
      remoteMinHint: 'Stellen außerhalb der Orte zählen erst ab so viel Remote.',
    },
    level: {
      a1: 'A1',
      a2: 'A2',
      b1: 'B1',
      b2: 'B2',
      c1: 'C1',
      c2: 'C2',
      native: 'Muttersprache',
    } satisfies Record<LanguageLevel, string>,
    remoteWish: {
      full: 'Voll',
      mostly: 'Überwiegend',
      partly: 'Teilweise',
      onSite: 'Vor Ort',
    } satisfies Record<RemoteWish, string>,
    availability: {
      unset: 'Keine Angabe',
      now: 'Sofort',
      from: 'Ab Datum',
    } satisfies Record<ProfileAvailability['kind'], string>,
    /** The countries the engine can tell apart in a job ad (ISO codes of `laender`). */
    country: {
      DE: 'Deutschland',
      AT: 'Österreich',
      CH: 'Schweiz',
      NL: 'Niederlande',
      FR: 'Frankreich',
      IT: 'Italien',
      ES: 'Spanien',
      PL: 'Polen',
      CZ: 'Tschechien',
      GB: 'Großbritannien',
      US: 'USA',
      IN: 'Indien',
    } as Record<string, string>,
    paste: {
      copied: 'Die Anfrage für Claude ist kopiert.',
      copyFailed: 'Die Anfrage ließ sich nicht kopieren.',
      copyAgain: 'Erneut kopieren',
      step: 'In Claude einfügen und den Lebenslauf anhängen.',
      answer: 'Antwort von Claude einfügen',
      take: 'Übernehmen',
    },
  },
  settings: {
    mailbox: 'Postfach',
    fetch: 'Abruf',
    portals: 'Portale',
    files: 'Dateien',
    maintenance: 'Wartung',
    connected: 'Verbunden',
    notConnected: 'Kein Postfach verbunden.',
    vault: {
      windowsCredentialManager:
        'Das App-Passwort liegt in der Windows-Anmeldeinformationsverwaltung.',
      macosKeychain: 'Das App-Passwort liegt im macOS-Schlüsselbund.',
    } satisfies Record<VaultKind, string>,
    address: 'Gmail-Adresse',
    password: 'App-Passwort',
    passwordHint: '16 Buchstaben, erstellt im Google-Konto.',
    createPassword: 'App-Passwort erstellen',
    twoStep: 'Ein App-Passwort gibt es nur mit der Bestätigung in zwei Schritten.',
    twoStepAction: 'Bestätigung einschalten',
    connect: 'Verbinden',
    removeMailbox: 'Postfach entfernen?',
    removeMailboxText: 'Das App-Passwort wird gelöscht, die Jobs bleiben.',
    autoFetch: 'Beim Start abrufen',
    autoFetchHint: 'Wenn der letzte Abruf mehr als sechs Stunden her ist.',
    autoArchive: 'Alte Jobs automatisch archivieren',
    autoArchiveHint: 'Nach 30 Tagen, außer Favoriten und Bewerbungen.',
    active: 'Aktiv',
    details: 'Details holen',
    needsDetails: 'Erst Details holen einschalten.',
    login: 'Mit Anmeldung',
    loginHint: 'Zeigt ganze Anzeigen statt eines Anrisses.',
    risk: {
      low: 'Geringes Risiko',
      grey: 'Graubereich',
      account: 'Kontorisiko',
    } satisfies Record<Risk, string>,
    riskText: {
      low: 'Nur öffentliche Seiten aus den eigenen Alert-Mails.',
      grey: 'Gastzugang, kein Konto ist betroffen.',
      account: 'Angemeldet steht das eigene Konto auf dem Spiel.',
    } satisfies Record<Risk, string>,
    /** What the risk word means (the badge's tooltip). */
    riskInfo: {
      low: 'Die App öffnet nur, was jeder im Browser sehen kann.',
      grey: 'Das Portal erlaubt automatisches Lesen nicht ausdrücklich.',
      account: 'Im schlimmsten Fall sperrt das Portal das eigene Konto.',
    } satisfies Record<Risk, string>,
    /** "Details holen" is off: what that changes. */
    detailsOff: 'Ohne Details bekommen die Jobs dieses Portals keine Passung.',
    quota: (used: number, cap: number) => `Heute ${n(used)} von ${n(cap)} Seiten`,
    quotaHour: (used: number, cap: number) => `Diese Stunde ${n(used)} von ${n(cap)} Seiten`,
    signedIn: 'Angemeldet',
    /** A sign-in still stored while the portal or its sign-in is switched off. */
    sessionLeft: 'Die Anmeldung ist noch gespeichert.',
    signedOut: 'Nicht angemeldet',
    signIn: 'Anmelden',
    signOut: 'Abmelden',
    openPortal: 'Im Browser öffnen',
    signInWaiting: 'Das Anmeldefenster ist offen.',
    workspace: 'Arbeitsordner',
    workspaceDefault: 'Standard',
    excel: 'Excel-Datei',
    excelMissing: 'Die Excel-Datei entsteht beim ersten Abruf.',
    txt: 'Textdateien',
    txtCount: (value: number) => count(value, 'Datei', 'Dateien'),
    txtNone: 'Es gibt noch keine Textdateien.',
    txtRewrite: 'Neu schreiben',
    txtClear: 'Löschen',
    txtWritten: (value: number) => `${count(value, 'Datei', 'Dateien')} geschrieben.`,
    txtFailed: (value: number) => `${count(value, 'Datei ist', 'Dateien sind')} gerade geöffnet.`,
    txtCleared: (value: number) => `${count(value, 'Datei', 'Dateien')} gelöscht.`,
    txtClearHeading: 'Textdateien löschen?',
    txtClearText: 'Beim nächsten Abruf entstehen sie neu.',
    fullMailbox: 'Ganzes Postfach lesen',
    fullMailboxHint: 'Liest alle Alert-Mails, nicht nur die neuen.',
    fullMailboxAction: 'Postfach lesen',
    fullMailboxHeading: 'Ganzes Postfach lesen?',
    fullMailboxText: 'Das dauert länger und ruft mehr Seiten der Portale ab.',
    logs: 'Protokolle',
    data: 'Daten der App',
    copyPath: 'Pfad kopieren',
    reset: 'Alles zurücksetzen',
    resetHint: 'Löscht Jobs, Einstellungen, Profil und App-Passwort.',
    resetAction: 'Zurücksetzen',
    resetHeading: 'Alles zurücksetzen?',
    resetText: 'Die App startet neu und ist danach leer.',
    resetDone: 'Die App ist zurückgesetzt.',
    resetPartly: (value: number) =>
      `Die App ist zurückgesetzt, ${count(value, 'Datei ließ', 'Dateien ließen')} sich nicht löschen.`,
    running: 'Ein Abruf läuft gerade.',
    dryRun: 'Probelauf, es werden keine Daten verändert.',
  },
  firstRun: {
    benefit: 'Die App liest die Alert-Mails aus Gmail und zeigt, welche Jobs zum Profil passen.',
    privacy: 'Alles bleibt auf diesem Rechner.',
    steps: 'Erste Schritte',
    mailbox: 'Postfach',
    mailboxText: 'An diese Gmail-Adresse müssen die Alert-Mails der Portale gehen.',
    profile: 'Profil',
    profileText: 'Das Profil entsteht in der App, auf Wunsch aus dem Lebenslauf.',
    fetch: 'Erster Abruf',
    fetchHint: 'Das dauert ein paar Minuten.',
  },
  shell: {
    loadFailed: 'Die App konnte ihre Daten nicht laden.',
    last: (iso: string) => `Zuletzt ${formatMoment(iso)}`,
    showRun: 'Abruf anzeigen',
    runFailed: 'Abruf fehlgeschlagen',
    /** Closing while a fetch runs: the window waits until it has stopped. */
    closing: 'Der Abruf wird beendet, dann schließt die App.',
  },
  toast: {
    saved: 'Gespeichert.',
    rescored: 'Die Jobs sind neu bewertet.',
    copied: 'Kopiert.',
    /** The job, or the best matches, as a prompt for any AI chat (no brand named). */
    prompt: 'Prompt kopiert, bereit für einen KI-Chat.',
    archivedOne: (name: string) => `„${name}“ archiviert.`,
    archivedMany: (value: number) => `${n(value)} Jobs archiviert.`,
    restored: (name: string) => `„${name}“ wiederhergestellt.`,
    deleted: (value: number) =>
      value === 1 ? 'Der Job ist gelöscht.' : `${n(value)} Jobs sind gelöscht.`,
    runDone: (value: number) =>
      value === 0
        ? 'Abruf fertig, nichts Neues.'
        : `Abruf fertig, ${count(value, 'neuer Job', 'neue Jobs')}.`,
    runDoneFilesOld: 'Abruf fertig, die Dateien sind nicht aktuell.',
  },
  error: {
    text: (kind: ErrorKind | 'unknown', params: Params): string => {
      if (kind === 'invalid' && typeof params.reason === 'string' && params.reason in invalid) {
        return textOf(invalid[params.reason as InvalidInput['reason']], params);
      }
      return textOf(errors[kind], params);
    },
  },
} as const;

export function textOf(text: Text, params: Params = {}): string {
  return typeof text === 'function' ? text(params) : text;
}

export type Catalog = typeof de;
