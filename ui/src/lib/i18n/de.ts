// German UI catalog - the source of UI text. en.ts says the same in English under the same
// keys (a missing or extra key there is a type error); the screens read the catalog of the
// app's language through `t` (t.ts).
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

/** The portals by their web address, everywhere (a sentence never starts with one). */
const portalName: Record<Portal, string> = {
  linkedin: 'linkedin.com',
  freelance: 'freelance.de',
  freelancermap: 'freelancermap.de',
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
  portalUnavailable: (p) => `Keine Verbindung zu ${portalOf(p.portal)}.`,
  portalPaused: (p) => `Die Abrufe bei ${portalOf(p.portal)} pausieren gerade.`,
  portalQuota: (p) => `Das Limit für ${portalOf(p.portal)} ist erreicht.`,
  internal: INTERNAL,
  unknown: INTERNAL,
};

/** Fields of the profile form, named in an error about their value (the label of the field
 *  without its unit, as everywhere: warnings, key names, the profile). */
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
  minDayRate: 'Mindest-Tagessatz',
  countries: 'Einsatzländer',
  contracts: 'Ausgeschlossene Vertragsarten',
  remoteOutside: 'Remote-Stellen im Ausland zulassen',
  available: 'Verfügbar ab',
  targetYears: 'Mindest-Erfahrung der Stelle',
  minSalary: 'Mindest-Jahresgehalt',
  permanentPlaces: 'Orte für Festanstellung',
  permanentRemoteMin: 'Mindest-Remote-Anteil',
  focus: 'Schwerpunkte',
  roles: 'Wunschrollen',
  wishDayRate: 'Wunschtagessatz',
  remote: 'Remote-Anteil',
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
  noSignIn: (p) => `Für ${portalOf(p.portal)} gibt es keine Anmeldung.`,
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
  country: (p): string =>
    p.allowed
      ? `Der Einsatzort liegt außerhalb von ${countryNames(p.allowed)}.`
      : 'Der Einsatzort passt nicht.',
  anueOptional: 'Arbeitnehmerüberlassung ist möglich, aber nicht Pflicht.',
  anueHidden: 'Die Anzeige deutet auf Arbeitnehmerüberlassung hin.',
  countryUnclear: 'Der Einsatzort ist unklar.',
  dayRateCurrency: (p) => `Der Satz ist in ${str(p.currency)} angegeben.`,
  availabilityGap: (p) =>
    `Der Start liegt ${count(num(p.days), 'Tag', 'Tage')} vor der Verfügbarkeit.`,
  startVague: 'Der Starttermin ist unklar.',
  permanent: (p) => {
    if (p.excluded !== true) return 'Das klingt nach einer Festanstellung.';
    return p.stated === true
      ? 'Die Stelle ist eine Festanstellung, das Profil schließt sie aus.'
      : 'Das klingt nach einer Festanstellung, das Profil schließt sie aus.';
  },
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
  /** Why a job is excluded by it, in the short words of a list row. */
  short: string;
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
    short: 'Tagessatz zu niedrig',
    exclusion: 'Der Tagessatz liegt unter dem Minimum im Profil.',
  },
  countries: {
    label: 'Einsatzländer',
    short: 'Einsatzort außerhalb',
    exclusion: 'Der Einsatzort liegt außerhalb der Länder im Profil.',
  },
  noAnue: {
    label: 'Arbeitnehmerüberlassung',
    short: 'Arbeitnehmerüberlassung',
    exclusion: ANUE,
  },
  noPermanent: {
    label: 'Festanstellung',
    short: 'Festanstellung',
    exclusion: 'Die Stelle ist eine Festanstellung, das Profil schließt sie aus.',
  },
  availability: {
    label: 'Verfügbarkeit',
    short: 'Start passt nicht',
    exclusion: 'Der Start passt nicht zur Verfügbarkeit.',
  },
  minSalary: {
    label: 'Jahresgehalt',
    short: 'Gehalt zu niedrig',
    exclusion: 'Das Gehalt liegt unter dem Minimum im Profil.',
  },
  permanentRegion: {
    label: 'Orte',
    short: 'Ort außerhalb der Region',
    exclusion: 'Die Festanstellung liegt außerhalb der Region im Profil.',
  },
  targetYears: {
    label: 'Erfahrung',
    short: 'Erfahrung passt nicht',
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

/** Names of profile keys the app speaks about (the keys themselves are an external contract):
 *  the criteria, wishes, Schwerpunkte and target roles, German and English, named like their
 *  field in the Profil form. */
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
  // `branchen` of the wishes (the engine reports no other one).
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
    .map((key) => `„${key}“`);
const joined = (items: string[]): string =>
  items.length < 2 ? items.join('') : `${items.slice(0, -1).join(', ')} und ${items.at(-1)}`;
/** ISO codes as the engine sends them (`DE, AT`) in words: "Deutschland und Österreich". */
const countryNames = (value: unknown): string =>
  joined(
    str(value)
      .split(',')
      .map((code) => code.trim())
      .filter((code) => code !== '')
      .map((code) => de.profile.country[code.toUpperCase()] ?? code),
  );

/** Profile warnings of the engine (`ProfileWarningCode`, core/src/matching/types.rs). */
const warning = {
  noCompetences: 'Das Profil nennt keine Kompetenzen.',
  fewCompetences: 'Das Profil nennt nur wenige Kompetenzen.',
  noCriteria: 'Das Profil setzt keine Ausschlusskriterien.',
  availabilityNotUnderstood: '„Verfügbar ab“ ist nicht lesbar.',
  // Keys of a criteria section the engine does not read (a typo, an unknown rule).
  ignoredKeys: (p) => `Die App liest ${joined(rawKeys(p.keys))} in den Ausschlusskriterien nicht.`,
  criterionNotUnderstood: (p) => `„${keyLabel(str(p.key))}“ ist nicht lesbar.`,
  regionWithoutPlaces: 'Der Mindest-Remote-Anteil wirkt nur zusammen mit Orten.',
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
    /** The arrow at the end of the Jobs row (kept); while a place is open it stays open. */
    hidePlaces: 'Archiv und Papierkorb ausblenden',
    showPlaces: 'Archiv und Papierkorb einblenden',
    placesStay: {
      archive: 'Bleibt offen, solange du im Archiv bist.',
      trash: 'Bleibt offen, solange du im Papierkorb bist.',
    },
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
    /** The tooltip of the handle, and its second line. */
    tip: 'Breite ändern',
    reset: 'Doppelklick setzt zurück',
  },
  /** The bar that replaces the list's second row while several jobs are selected. */
  selection: {
    count: (n: number) => `${n} ausgewählt`,
    clear: 'Auswahl aufheben',
    /** The reader while several jobs are chosen. */
    chosen: (value: number) => `${count(value, 'Job', 'Jobs')} ausgewählt`,
    /** The key that takes a row in or out, by OS. */
    commandKey: { ctrl: 'Strg', cmd: 'Cmd' } satisfies Record<'ctrl' | 'cmd', string>,
    hint: (key: string) =>
      `${key}+Klick nimmt einen Job dazu oder heraus, Umschalt+Klick einen ganzen Bereich.`,
    /** Once, after a few single moves: several jobs can go at once. */
    tip: (key: string) => `Mehrere Jobs auf einmal wählst du mit ${key}+Klick.`,
  },
  /** Where a job is, like a mail: the inbox ("Jobs" in the sidebar), the archive, the trash. */
  place: {
    /** The inbox is "Jobs", like the sidebar says. */
    inbox: 'Jobs',
    archive: 'Archiv',
    trash: 'Papierkorb',
    /** The field's placeholder names what it searches. */
    search: {
      inbox: 'Jobs durchsuchen',
      archive: 'Archiv durchsuchen',
      trash: 'Papierkorb durchsuchen',
    } satisfies Record<Place, string>,
    /** The second header row of the archive and the trash. */
    count: {
      inbox: (value: number) => `${count(value, 'Job', 'Jobs')} unter Jobs`,
      archive: (value: number) => `${count(value, 'Job', 'Jobs')} im Archiv`,
      trash: (value: number) => `${count(value, 'Job', 'Jobs')} im Papierkorb`,
    } satisfies Record<Place, (value: number) => string>,
    /** The same row during a search: what it found there, not how many jobs lie there. */
    found: {
      inbox: (value: number, query: string) =>
        `${count(value, 'Job', 'Jobs')} zu „${query}“ unter Jobs`,
      archive: (value: number, query: string) =>
        `${count(value, 'Job', 'Jobs')} zu „${query}“ im Archiv`,
      trash: (value: number, query: string) =>
        `${count(value, 'Job', 'Jobs')} zu „${query}“ im Papierkorb`,
    } satisfies Record<Place, (value: number, query: string) => string>,
    /** Search hits in another place, under the results. */
    alsoIn: {
      inbox: (value: number) => `Auch unter Jobs (${n(value)})`,
      archive: (value: number) => `Auch im Archiv (${n(value)})`,
      trash: (value: number) => `Auch im Papierkorb (${n(value)})`,
    } satisfies Record<Place, (value: number) => string>,
    /** The quiet line under the title of a job that is not in the inbox. */
    inArchive: 'Im Archiv',
    inTrash: 'Im Papierkorb',
    inTrashFor: (days: number) =>
      `Im Papierkorb, wird nach ${count(days, 'Tag', 'Tagen')} gelöscht`,
    empty: {
      inbox: 'Keine Jobs.',
      archive: 'Das Archiv ist leer.',
      trash: 'Der Papierkorb ist leer.',
    } satisfies Record<Place, string>,
    /** The reader of the archive and the trash while no job is open. */
    reader: {
      inbox: 'Wähle einen Job aus der Liste.',
      archive: 'Archivierte Jobs bleiben hier, bis du sie zurückholst oder löschst.',
      trash: 'Gelöschte Jobs liegen hier, bis du sie wiederherstellst oder den Papierkorb leerst.',
    } satisfies Record<Place, string>,
    trashFor: (days: number) =>
      `Gelöschte Jobs liegen hier ${count(days, 'Tag', 'Tage')}, dann sind sie endgültig weg.`,
  },
  /** What a job can do where it is: one name and icon on a row, in the reader, in the bar. */
  actions: {
    archive: 'Archivieren',
    toInbox: 'Zurück zu Jobs',
    trash: 'In den Papierkorb',
    restore: 'Wiederherstellen',
    purge: 'Endgültig löschen',
    purgeHeading: (value: number) =>
      value === 1 ? 'Job endgültig löschen?' : `${n(value)} Jobs endgültig löschen?`,
    purgeText: 'Gelöschte Jobs kommen nicht wieder, auch nicht mit alten Alert-Mails.',
    emptyTrash: 'Papierkorb leeren',
    emptyTrashHeading: 'Papierkorb leeren?',
    emptyTrashText: (value: number) =>
      value === 1
        ? 'Der Job wird endgültig gelöscht und kommt nicht wieder.'
        : `Die ${n(value)} Jobs werden endgültig gelöscht und kommen nicht wieder.`,
    markAllRead: 'Alle als gelesen markieren',
  },
  /** The native context menu of fields and selected text (the OS's words). */
  edit: {
    undo: 'Rückgängig',
    cut: 'Ausschneiden',
    copy: 'Kopieren',
    paste: 'Einfügen',
    delete: 'Löschen',
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
      partial ? `Passt teilweise zu „${profile}“ im Profil.` : `Passt zu „${profile}“ im Profil.`,
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
      pending: 'Details folgen',
      teaser: 'Nur Anriss',
      failed: 'Details fehlen',
      unfetchable: 'Nicht abrufbar',
      gone: 'Nicht mehr online',
      onRequest: 'Details auf Anfrage',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    /** What a detail badge means, in its tooltip. */
    detailHint: {
      pending: 'Die ganze Anzeige ist noch nicht geholt.',
      teaser: 'Das Portal zeigt ohne Anmeldung nur einen Anriss.',
      failed: 'Die ganze Anzeige ließ sich nicht holen.',
      unfetchable: 'Die Anzeige ließ sich mehrmals nicht lesen.',
      gone: 'Die Anzeige ist nicht mehr online.',
      onRequest: 'Bei älteren Jobs kommen die Details nur auf Anfrage.',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    /** The ad's page says it takes no applications any more (badge and its tooltip). */
    closed: 'Keine Bewerbung mehr möglich',
    closedHint: 'Die Anzeige ist noch lesbar, nimmt aber keine Bewerbungen mehr an.',
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
      match: 'Nach Passung',
      newest: 'Nach Datum',
    } satisfies Record<JobSort, string>,
    /** The order without a usable profile: there is no fit to sort by. */
    sortNoProfile: 'Ohne Profil nur nach Datum.',
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
      fullMailbox: 'Ältere Mails lesen',
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
          return `Pause bei ${name}`;
        case 'quotaReached':
          return `Limit bei ${name} erreicht`;
        case 'layoutSuspect':
          return `Seiten von ${name} sehen anders aus als erwartet`;
        case 'loginRequired':
          return `Anmeldung bei ${name} nötig`;
      }
    },
    checkMailbox: 'Postfach prüfen',
  },
  list: {
    label: 'Jobs',
    /** The divider (its count is a pill of its own, left out where the rows are a part). */
    excluded: 'Ausgeschlossen',
    /** The empty list says where jobs come from and how to get more. */
    emptySources: 'Ein Alert pro Portal bringt neue Jobs.',
    /** FR-03: while the first fetch runs, the empty list only says what comes. */
    emptyWhileRun: 'Die Jobs erscheinen, sobald der Abruf fertig ist.',
    createAlert: (portal: string) => `Alert auf ${portal} anlegen`,
    readOlder: 'Ältere Mails lesen',
    emptyNew: 'Keine neuen Jobs.',
    emptyFavourites: 'Noch keine Favoriten.',
    emptyAll: 'Nach dem ersten Abruf stehen die Jobs hier.',
    emptyAfterRun: 'Die Alert-Mails enthielten bisher keine Jobs.',
    noHit: (query: string) => `Keine Jobs zu „${query}“.`,
    /** A search under Neu or Favoriten that Alle would find. */
    noHitIn: {
      new: (query: string) => `Keine neuen Jobs zu „${query}“.`,
      favourites: (query: string) => `Keine Favoriten zu „${query}“.`,
    },
    searchAll: 'In allen suchen',
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
    fullRemote: 'voll remote',
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
    /** An excluded job the user counts anyway, and back. */
    override: 'Trotzdem werten',
    overrideUndo: 'Wieder ausschließen',
    overridden: 'Von dir als passend markiert.',
    prompt: 'Prompt für KI-Bewertung kopieren',
    promptShort: 'Prompt kopieren',
    promptHint:
      'Kopiert Anzeige und Profil als fertigen Prompt für ChatGPT, Claude oder eine andere KI.',
    /** Under the band of a score that comes from a teaser only. */
    preliminary: 'Vorläufig, aus einem Anriss bewertet',
    mail: OPEN_MAIL,
    noMail: 'Zu diesem Job gibt es keine Alert-Mail.',
    /** The teaser note names the portal; the sign-in is set up in Einstellungen. */
    teaserOf: (portal: string) => `Ohne Anmeldung zeigt ${portal} nur einen Anriss.`,
    setUpSignIn: 'Anmeldung einrichten',
    promptNoProfile: 'Ohne Profil gibt es nichts zu bewerten.',
    promptNoText: 'Der Text der Anzeige fehlt noch.',
    /** The exact moment of the mail, in the tooltip of its date. */
    mailAt: (moment: string) => `Alert-Mail vom ${moment}`,
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
      unfetchable: 'Die Anzeige ließ sich mehrmals nicht lesen.',
      gone: 'Die Anzeige ist nicht mehr online.',
      onRequest: 'Bei älteren Jobs kommen die Details nur auf Anfrage.',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    closed: 'Die Anzeige nimmt keine Bewerbungen mehr an.',
    detailsOff: 'Details holen ist für dieses Portal aus.',
    short: SHORT_TEXT,
    loadFailed: 'Der Job ließ sich nicht laden.',
  },
  overview: {
    noProfileText: 'Mit einem Profil zeigt jeder Job, wie gut er passt.',
    profileUnreadable: 'Profil nicht lesbar',
    label: 'Tagesüberblick',
    /** Shown in the empty reader when the overview has nothing else to say (like Mail's "no message selected"). */
    pick: 'Links einen Job auswählen.',
    issues: 'Offene Punkte',
    best: 'Neu und passend',
    excel: 'Excel öffnen',
    /** The best matches as one prompt for any AI chat. */
    promptTop: 'Prompt für KI-Vergleich kopieren',
    promptTopNone: 'Noch kein Job bewertet.',
    /** When the list beside shows the best new jobs on top already. */
    bestInList: 'Die besten neuen Jobs stehen oben in der Liste.',
    files: 'Dateien',
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
    /** Under the error of a profile that no longer reads. */
    replaces: 'Ein neues Profil ersetzt die Datei.',
    create: 'Profil anlegen',
    fromCv: 'Aus Lebenslauf erstellen',
    /** The same way for a profile that exists: the answer fills the form for review. */
    updateFromCv: 'Aus Lebenslauf aktualisieren',
    pick: 'Datei wählen',
    pickOther: 'Andere Datei wählen',
    remove: 'Entfernen',
    removeHeading: 'Profil entfernen?',
    removeText:
      'Die Jobs zeigen danach keine Passung mehr. Die Datei bleibt als Sicherung im Profilordner.',
    removed: 'Profil entfernt.',
    savedAt: (date: string, time: string) => `Gespeichert ${date}, ${time}`,
    unnamed: 'Profil ohne Namen',
    quality: {
      good: 'Vollständig',
      thin: 'Wenig Inhalt',
      empty: 'Ohne Kompetenzen',
    } satisfies Record<ProfileQuality, string>,
    qualityText: {
      good: 'Die Passung stützt sich auf das ganze Profil.',
      thin: 'Wenige Kompetenzen, die Passung bleibt grob.',
      empty: 'Ohne Kompetenzen wird nichts bewertet.',
    } satisfies Record<ProfileQuality, string>,
    rescoring: (value: number) => `${count(value, 'Job wird', 'Jobs werden')} neu bewertet.`,
    rescored: 'Gespeichert, Jobs neu bewertet.',
    /** The badge of a well filled profile with a value to check (its tooltip says which). */
    check: 'Etwas prüfen',
    next: 'Weiter zum ersten Abruf',
    understood: (terms: number) => `${count(terms, 'Begriff', 'Begriffe')} für die Passung`,
    focusCount: (focus: number) => count(focus, 'Schwerpunkt', 'Schwerpunkte'),
    /** The domain packs the profile switched on, by their names. */
    packs: (packs: string[]) => `Fachwortschatz für ${joined(packs)}`,
    warning,
    /** Every domain pack of the engine (core/src/matching/lexicon/domains). */
    pack: {
      finance: 'Finanzen',
      sap: 'SAP',
      itProject: 'IT-Projekte',
      hr: 'Personal',
      procurement: 'Einkauf',
      data: 'Daten',
      pharma: 'Pharma',
      operations: 'Produktion',
      sales: 'Vertrieb',
      legal: 'Recht',
      software: 'Software',
    } as Record<string, string>,
    draft: {
      new: 'Neues Profil',
      file: 'Profil aus einer Datei',
      answer: 'Profil aus dem Lebenslauf',
      update: 'Profil mit dem Lebenslauf aktualisiert',
    },
    unsaved: 'Nicht gespeichert',
    review: 'Die Angaben prüfen, dann speichern.',
    save: 'Speichern',
    discard: 'Verwerfen',
    saved: 'Gespeichert.',
    leaveHeading: 'Änderungen speichern?',
    leaveText: 'Die Änderungen am Profil sind nicht gespeichert.',
    empty: 'Noch leer',
    section: {
      person: 'Person',
      competences: 'Kompetenzen und Schwerpunkte',
      experience: 'Erfahrung und Qualifikation',
      wishes: 'Wünsche',
      criteria: 'Ausschlusskriterien',
      permanent: 'Festanstellung',
      availability: 'Verfügbarkeit',
      understood: 'So liest die App dein Profil',
    },
    sectionHint: {
      wishes: 'Wünsche verschieben die Bewertung leicht, sie schließen nichts aus.',
      criteria: 'Ein Job, der hier nicht passt, gilt als ausgeschlossen.',
      permanent: 'Diese Regeln gelten nur für Festanstellungen.',
      availability:
        'Beginnt ein Job früher, markiert die App ihn zum Prüfen, sie schließt ihn nicht aus.',
    },
    field: {
      name: 'Name',
      title: 'Rolle',
      titleHint: 'Die Rolle zählt für die Passung.',
      titlePlaceholder: 'Projektleitung',
      roles: 'Wunschrollen',
      rolesHint: 'Passt der Titel einer Anzeige dazu, steigt die Bewertung leicht.',
      rolesPlaceholder: 'Teamleitung',
      competence: 'Kompetenz',
      competencePlaceholder: 'Projektmanagement',
      years: 'Jahre',
      yearsHint: 'Die Jahre zählen, wenn eine Anzeige Erfahrung in Jahren verlangt.',
      aliases: 'Andere Begriffe',
      aliasesHint: 'Synonyme oder englische Begriffe.',
      addCompetence: 'Kompetenz hinzufügen',
      removeCompetence: (name: string) => `${name || 'Kompetenz'} entfernen`,
      star: 'Als Schwerpunkt markieren',
      focusCount: (count: number, max: number) => `Schwerpunkte ${count} von ${max}`,
      focusHint: 'Kompetenzen mit Stern zählen doppelt, höchstens fünf.',
      focusFull: 'Höchstens fünf Schwerpunkte.',
      /** More Schwerpunkte in a file or an answer than count. */
      focusTrimmed: (count: number) =>
        `Die Datei nennt ${n(count)} Schwerpunkte, übernommen sind die ersten fünf.`,
      strengths: 'Besondere Stärken',
      strengthsHint: 'Sie stützen die Passung, belegen aber keine Anforderung.',
      strengthsPlaceholder: 'Große Projekte im Zeitplan übergeben',
      keywords: 'Stichworte',
      keywordsPlaceholder: 'Agil, Change Management',
      keywordsHint: 'Begriffe, die in passenden Anzeigen stehen.',
      totalYears: 'Berufserfahrung (Jahre)',
      totalYearsHint: 'Ab zehn Jahren bewertet die App Einstiegsstellen niedrig.',
      degrees: 'Abschlüsse',
      degreesPlaceholder: 'Master of Science',
      industries: 'Branchen',
      industriesPlaceholder: 'Maschinenbau',
      tools: 'Werkzeuge und Methoden',
      toolsPlaceholder: 'Microsoft Excel',
      certificates: 'Zertifikate',
      certificatesPlaceholder: 'PMP',
      languages: 'Sprachen',
      language: 'Sprache',
      languagePlaceholder: 'Englisch',
      level: 'Niveau',
      levelHint: 'Ohne Niveau rechnet die App mit B2.',
      addLanguage: 'Sprache hinzufügen',
      removeLanguage: (name: string) => `${name || 'Sprache'} entfernen`,
      wishRate: 'Wunschtagessatz (€)',
      wishRateHint: 'Den Mindest-Tagessatz legen die Ausschlusskriterien fest.',
      remote: 'Remote-Anteil',
      regions: 'Wunschregionen',
      regionsPlaceholder: 'München',
      wishIndustries: 'Wunschbranchen',
      wishIndustriesPlaceholder: 'Gesundheitswesen',
      minDayRate: 'Mindest-Tagessatz (€)',
      minDayRateHint: 'Liegt der Satz einer Anzeige darunter, fällt der Job weg.',
      countries: 'Einsatzländer',
      remoteOutside: 'Remote-Stellen im Ausland zulassen',
      remoteOutsideHint:
        'Ausgeschaltet markiert die App ganz remote Stellen mit Sitz im Ausland zum Prüfen.',
      remoteOutsideOff: 'Erst Einsatzländer wählen.',
      noAnue: 'Arbeitnehmerüberlassung ausschließen',
      noPermanent: 'Festanstellung ausschließen',
      noPermanentHint: 'Nur bei klarem Wortlaut, sonst markiert die App den Job zum Prüfen.',
      available: 'Verfügbar ab',
      date: 'Datum',
      datePlaceholder: '01.11.2026',
      dateInvalid: 'Datum im Format 01.11.2026 eingeben.',
      targetYears: 'Mindest-Erfahrung der Stelle (Jahre)',
      targetYearsHint: 'Stellen für deutlich weniger Erfahrung fallen weg.',
      minSalary: 'Mindest-Jahresgehalt (€)',
      places: 'Orte für Festanstellung',
      placesPlaceholder: 'München',
      remoteMin: 'Mindest-Remote-Anteil (%)',
      remoteMinHint:
        'Außerhalb dieser Orte zählt eine Festanstellung erst ab diesem Remote-Anteil.',
      /** A euro amount with cents: the app counts whole euros. */
      rounded: 'Auf ganze Euro abgerundet.',
      /** A value in the file that the app could not read, shown at its field. */
      unreadableNumber: (value: string) => `In der Datei stand „${value}“, das ist keine Zahl.`,
      unreadableDate: (value: string) => `In der Datei stand „${value}“, das ist kein Datum.`,
      unreadableValue: (value: string) =>
        `In der Datei stand „${value}“, das kann die App nicht lesen.`,
      unreadableFocus: (value: string) => `„${value}“ steht nicht bei den Kompetenzen.`,
      unreadableRole: (value: string) => `„${value}“ nennt kein Fachgebiet.`,
      removeValue: 'Wert entfernen',
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
    /** What a level means, in the tooltip of its button. */
    levelMeaning: {
      a1: 'Anfänger',
      a2: 'Grundkenntnisse',
      b1: 'Mittelstufe',
      b2: 'Gute Kenntnisse',
      c1: 'Fließend',
      c2: 'Verhandlungssicher',
      native: 'Muttersprache',
    } satisfies Record<LanguageLevel, string>,
    remoteWish: {
      full: 'Ganz remote',
      mostly: 'Überwiegend remote',
      partly: 'Teilweise remote',
      onSite: 'Vor Ort',
    } satisfies Record<RemoteWish, string>,
    /** Nothing chosen means no availability (pressing the chosen one again clears it). */
    availability: {
      now: 'Sofort',
      from: 'Ab Datum',
    } satisfies Record<Exclude<ProfileAvailability['kind'], 'unset'>, string>,
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
    /** "So liest die App dein Profil": what the engine reads in the file. */
    reading: {
      terms: (value: number) =>
        `${count(value, 'Begriff zählt', 'Begriffe zählen')} für die Passung.`,
      termsLabel: 'Begriffe',
      more: (value: number) => `und ${n(value)} weitere`,
      sources: 'Gelesen aus',
      /** A part of the file the form does not show (career stations and the like). */
      fileOnly: (name: string) => `${name}, nur in der Datei`,
      years: 'Berufserfahrung',
      yearsValue: (value: number) => count(value, 'Jahr', 'Jahre'),
      degrees: 'Abschlüsse',
      packs: 'Fachwortschatz',
      criteria: 'Ausschlusskriterien',
      none: 'Keine',
      from: (value: string) => `ab ${value}`,
      excluded: 'ausgeschlossen',
      stale: 'Das gilt für den gespeicherten Stand.',
      /** Parts of the profile file by their key (an external contract), in the form's words. */
      source: {
        titel: 'Rolle',
        kernkompetenzen: 'Kompetenzen',
        methoden_tools: 'Werkzeuge und Methoden',
        zertifizierungen: 'Zertifikate',
        branchen: 'Branchen',
        sprachen: 'Sprachen',
        alleinstellungsmerkmale: 'Besondere Stärken',
        keywords: 'Stichworte',
        abschluss: 'Abschlüsse',
        ausbildung: 'Abschlüsse',
        schwerpunkte: 'Schwerpunkte',
        stationen: 'Stationen',
        projekte: 'Projekte',
      } as Record<string, string>,
    },
    paste: {
      privacy: 'Der Lebenslauf geht an die KI, die du nutzt.',
      copied: 'Der Prompt ist kopiert.',
      copyFailed: 'Der Prompt ließ sich nicht kopieren.',
      copy: 'Prompt kopieren',
      copyAgain: 'Erneut kopieren',
      step: 'In eine KI einfügen und den Lebenslauf anhängen.',
      preview: 'Prompt ansehen',
      answer: 'Antwort der KI',
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
    /** The last fetch could not reach Gmail, or Gmail refused the password. */
    unreachable: 'Nicht erreichbar',
    refused: 'Abgelehnt',
    mailRefused: 'Gmail lehnt Adresse oder App-Passwort ab, bitte über Ändern neu eintragen.',
    vault: {
      windowsCredentialManager:
        'Das App-Passwort liegt in der Windows-Anmeldeinformationsverwaltung.',
      macosKeychain: 'Das App-Passwort liegt im macOS-Schlüsselbund.',
    } satisfies Record<VaultKind, string>,
    address: 'Gmail-Adresse',
    password: 'App-Passwort',
    passwordHint: '16 Buchstaben, erstellt im Google-Konto.',
    createPassword: 'App-Passwort erstellen',
    twoStep: 'Ein App-Passwort braucht die Bestätigung in zwei Schritten.',
    addressMissing: 'Die Gmail-Adresse fehlt.',
    passwordMissing: 'Das App-Passwort fehlt.',
    twoStepAction: 'Bestätigung einschalten',
    connect: 'Verbinden',
    removeMailbox: 'Postfach entfernen?',
    removeMailboxText: 'Das App-Passwort wird gelöscht, die Jobs bleiben.',
    autoFetch: 'Beim Start abrufen',
    autoFetchHint: 'Wenn der letzte Abruf mehr als sechs Stunden her ist.',
    autoArchive: 'Jobs nach 30 Tagen archivieren',
    autoArchiveHint: 'Favoriten werden nie archiviert.',
    autoEmptyTrash: 'Papierkorb nach 30 Tagen leeren',
    autoEmptyTrashHint: 'Gelöschte Jobs sind danach endgültig weg.',
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
    /** The sign-in row of a portal: its label, and its state. */
    session: 'Anmeldung',
    signedIn: 'Angemeldet',
    notSignedIn: 'Nicht angemeldet.',
    /** A portal that is off. */
    portalOff: 'Wird beim Abruf übersprungen.',
    /** A sign-in still stored while the portal or its sign-in is switched off. */
    sessionLeft: 'Die Anmeldung ist noch gespeichert.',
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
    /** The dialog's confirm: the bare verb of its heading, like every dialog. */
    fullMailboxConfirm: 'Lesen',
    fullMailboxHeading: 'Ganzes Postfach lesen?',
    fullMailboxText: 'Das dauert länger und ruft mehr Seiten der Portale ab.',
    logs: 'Protokolle',
    data: 'Daten der App',
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
    language: 'Sprache',
    languageLabel: 'Sprache der App',
    /** Excel file and overview are written at the next fetch (the text files stay German). */
    languageHint: 'Excel-Datei und Übersicht folgen beim nächsten Abruf.',
    /** Each language in its own words, in both catalogs. */
    languageName: {
      de: 'Deutsch',
      en: 'English',
    } satisfies Record<Language, string>,
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
    last: (iso: string) => `Abgerufen ${formatMoment(iso)}`,
    showRun: 'Abruf anzeigen',
    runFailed: (iso: string) => `Fehlgeschlagen ${formatMoment(iso)}`,
    /** Closing while a fetch runs: the window waits until it has stopped. */
    closing: 'Der Abruf wird beendet, dann schließt die App.',
    /** The sidebar's edge: what a click does, over the key that does the same (by OS). */
    collapseSidebar: 'Seitenleiste einklappen',
    expandSidebar: 'Seitenleiste ausklappen',
    sidebarKey: { ctrl: 'Strg+B', cmd: '⌘B' } satisfies Record<'ctrl' | 'cmd', string>,
  },
  toast: {
    saved: 'Gespeichert.',
    mailboxSaved: 'Postfach verbunden.',
    rescored: 'Die Jobs sind neu bewertet.',
    copied: 'Kopiert.',
    /** The job, or the best matches, as a prompt for any AI chat (no brand named). */
    prompt: 'Prompt kopiert, bereit für einen KI-Chat.',
    archivedOne: (name: string) => `„${name}“ archiviert.`,
    trashedOne: (name: string) => `„${name}“ in den Papierkorb gelegt.`,
    trashedMany: (value: number) => `${n(value)} Jobs in den Papierkorb gelegt.`,
    inboxOne: (name: string) => `„${name}“ zurückgeholt.`,
    inboxMany: (value: number) => `${n(value)} Jobs zurückgeholt.`,
    restoredMany: (value: number) => `${n(value)} Jobs wiederhergestellt.`,
    allRead: 'Alle als gelesen markiert.',
    archivedMany: (value: number) => `${n(value)} Jobs archiviert.`,
    restored: (name: string) => `„${name}“ wiederhergestellt.`,
    /** Only a deletion for good says "endgültig". */
    deleted: (value: number) =>
      value === 1 ? 'Der Job ist endgültig gelöscht.' : `${n(value)} Jobs sind endgültig gelöscht.`,
    trashEmptied: 'Papierkorb geleert.',
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

/** The shape of this catalog with any words: the type of every catalog (en.ts). */
export type Catalog = Widen<typeof de>;

/** Words become `string`; keys, nesting and function signatures stay. */
type Widen<T> = T extends string
  ? string
  : T extends (...args: infer A) => infer R
    ? (...args: A) => Widen<R>
    : T extends object
      ? { -readonly [K in keyof T]: Widen<T[K]> }
      : T;
