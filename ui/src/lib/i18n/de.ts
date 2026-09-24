// German UI catalog - the only source of UI text.
//
// Style rules (CLAUDE.md, checked by core/tests/ui_contract.rs): little text, plain and
// human. Buttons are one verb phrase without a period; notes are one short sentence with a
// period; headings and labels end without a colon; no dash or em dash as a separator, no
// "X: Y", no exclamation marks, no text twice. Glossary: Job · Portal · Passung · Details ·
// Abrufen · Profil · Postfach · Alert-Mail · Übersicht · Ausgeschlossen · Neu · Zu prüfen ·
// Merken.
//
// Every code of the generated types has exactly one text here: the tables are typed as
// `Record<Code, ...>`, so a new code without a text is a type error.

import type {
  Band,
  DetailState,
  ErrorKind,
  InvalidInput,
  PauseReason,
  Portal,
  ProfileQuality,
  ReasonKind,
  ReasonWeight,
  Risk,
  RunKindName,
  StatusCode,
  Step,
  VaultKind,
  WorkMode,
} from '../ipc/types';
import {
  formatCountdown,
  formatDate,
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

const INTERNAL = 'Ein interner Fehler, Details stehen im Protokoll.';

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

const invalid: Record<InvalidInput['reason'], Text> = {
  noPortal: 'Mindestens ein Portal muss aktiv sein.',
  profileNotUtf8: 'Die Datei ist keine Textdatei.',
  profileNotJson: (p) => `Die Datei ist kein gültiges JSON, Zeile ${str(p.line)}.`,
  profileNotObject: 'Die Datei enthält kein Profil.',
  mailAddress: 'Die Adresse ist unvollständig.',
  appPassword: 'Ein App-Passwort hat 16 Buchstaben.',
  noSignIn: (p) => `${portalOf(p.portal)} bietet keine Anmeldung.`,
};

const status: Record<StatusCode, string> = {
  connectingMail: 'Verbindet mit dem Postfach',
  searchingMail: 'Sucht Alert-Mails',
  readingMails: 'Liest Alert-Mails',
  fetchingDetails: 'Holt Details',
  signingIn: 'Meldet an',
  waiting: 'Wartet auf das Portal',
  scoring: 'Bewertet die Jobs',
  writingFiles: 'Schreibt die Dateien',
};

const pause: Record<PauseReason, string> = {
  throttled: 'Das Portal bremst die Abrufe.',
  blocked: 'Das Portal blockiert die Abrufe.',
  layoutChanged: 'Die Seiten sehen anders aus als erwartet.',
  stateUnreadable: 'Der Stand des Portals ist nicht lesbar.',
  network: 'Das Portal ist nicht erreichbar.',
  challenged: 'Das Portal verlangt eine Prüfung.',
};

const ANUE = 'Die Anzeige nennt Arbeitnehmerüberlassung.';
const LOW_TEXT = 'Die Anzeige hat wenig Text.';
const SHORT_TEXT = 'Die Anzeige ist sehr kurz.';

/** Contract type of an ad (`contractType` params `type`, `inferred`). */
const contract = {
  interim: 'Interim',
  permanent: 'Festanstellung',
  anue: 'ANÜ',
  unclear: 'Vertragsart unklar',
} as const;
export type ContractKind = keyof typeof contract;

function contractName(p: Params): string {
  const type = typeof p.type === 'string' && p.type in contract ? (p.type as ContractKind) : null;
  if (type === null) return contract.unclear;
  return p.inferred && type !== 'unclear' ? `Vermutlich ${contract[type]}` : contract[type];
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
  availabilityGap: (p) => `Der Start liegt ${n(num(p.days))} Tage vor der Verfügbarkeit.`,
  startVague: 'Der Starttermin ist offen.',
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
      ? `Die Stelle verlangt ${n(num(p.years))} Jahre Erfahrung, das Profil zielt auf ${n(num(p.target))}.`
      : 'Die Stelle richtet sich an weniger Erfahrene.',
  seniorityUnclear: (p) =>
    p.junior
      ? 'Der Titel klingt nach einer Einstiegsstelle.'
      : 'Das gesuchte Erfahrungslevel ist unklar.',
  overqualified: (p) =>
    p.years !== undefined && p.years !== null
      ? `Gesucht sind ${n(num(p.years))} Jahre Erfahrung, das Profil bringt deutlich mehr mit.`
      : 'Das Profil ist deutlich erfahrener als gesucht.',
  contractType: (p) => contractName(p),
  formalOpen: (p) => {
    if (p.class === undefined || p.class === null) return 'Das Profil nennt keinen Abschluss.';
    const what = p.class === 'licence' ? 'eine Zulassung' : 'einen Abschluss';
    return p.mandatory
      ? `Die Anzeige verlangt ${what}, den das Profil nicht nennt.`
      : `Die Anzeige wünscht ${what}, den das Profil nicht nennt.`;
  },
  lowEvidence: LOW_TEXT,
  shortText: SHORT_TEXT,
} satisfies Record<string, Text>;
export type ReasonCode = keyof typeof reasonCode;

interface CriterionText {
  /** Short name in the criteria strip of the reader. */
  label: string;
  /** In the profile, when set (params carry the value). */
  set: (p: Params) => string;
  /** In the profile, when not set. */
  unset: string;
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
    set: (p) => `Tagessatz ab ${formatEuro(p.min ?? p.rate ?? p.value)}`,
    unset: 'Kein Mindest-Tagessatz',
    exclusion: 'Der Tagessatz liegt unter dem Minimum im Profil.',
  },
  countries: {
    label: 'Einsatzland',
    set: (p) => `Einsatz nur in ${str(p.countries ?? p.value)}`,
    unset: 'Kein Einsatzland festgelegt',
    exclusion: 'Der Einsatzort liegt außerhalb der Länder im Profil.',
  },
  noAnue: {
    label: 'ANÜ',
    set: () => 'Keine Arbeitnehmerüberlassung',
    unset: 'Arbeitnehmerüberlassung ist erlaubt',
    exclusion: ANUE,
  },
  availability: {
    label: 'Verfügbarkeit',
    set: (p) => {
      const from = p.from ?? p.value;
      if (from === 'now') return 'Sofort verfügbar';
      const date = typeof from === 'string' ? formatDate(from) : '';
      return date ? `Verfügbar ab ${date}` : 'Verfügbarkeit angegeben';
    },
    unset: 'Keine Verfügbarkeit angegeben',
    exclusion: 'Der Start passt nicht zur Verfügbarkeit.',
  },
  minSalary: {
    label: 'Gehalt',
    set: (p) => `Festanstellung ab ${formatEuro(p.min ?? p.value)} im Jahr`,
    unset: 'Kein Mindestgehalt für Festanstellungen',
    exclusion: 'Das Gehalt liegt unter dem Minimum im Profil.',
  },
  permanentRegion: {
    label: 'Region',
    set: (p) =>
      typeof p.remoteMin === 'number' && p.remoteMin > 0
        ? `Festanstellung in ${str(p.places)} oder ab ${formatPercent(p.remoteMin)} remote`
        : `Festanstellung nur in ${str(p.places)}`,
    unset: 'Keine Region für Festanstellungen',
    exclusion: 'Die Festanstellung liegt außerhalb der Region im Profil.',
  },
  targetYears: {
    label: 'Seniorität',
    set: (p) => `Stellen ab ${n(num(p.min ?? p.value))} Jahren Erfahrung`,
    unset: 'Kein Mindestlevel für Stellen',
    exclusion: 'Die Stelle verlangt deutlich weniger Erfahrung.',
  },
} satisfies Record<string, CriterionText>;
export type CriterionKey = keyof typeof criteria;

export type CriterionState = 'met' | 'violated' | 'unknown' | 'unset';

/** `JobMatch.note` / `MatchDetail.summary` codes. */
const note = {
  hardCriterion: 'Ein Ausschlusskriterium greift.',
  fewMust: 'Wenige Muss-Anforderungen erfüllt.',
  shortText: 'Zu wenig Text für eine Bewertung.',
  lowEvidence: LOW_TEXT,
} satisfies Record<string, Text>;
export type MatchNote = keyof typeof note;

/** Profile warnings of the engine, plus the keys the app does not evaluate. */
const warning = {
  noCompetences: 'Das Profil nennt keine Kompetenzen.',
  fewCompetences: 'Das Profil nennt nur wenige Kompetenzen.',
  noCriteria: 'Das Profil setzt keine Ausschlusskriterien.',
  availabilityNotUnderstood: 'Die Verfügbarkeit im Profil ist nicht lesbar.',
  ignoredKeys: (p) => `Nicht ausgewertet ${str(p.keys)}.`,
  criterionNotUnderstood: (p) => `Der Wert von ${str(p.key)} ist nicht lesbar.`,
  regionWithoutPlaces: 'Für die Region fehlen die Orte, die Regel bleibt aus.',
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
  window: {
    minimize: 'Minimieren',
    maximize: 'Maximieren',
    restore: 'Verkleinern',
    close: 'Schließen',
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
    openFolder: 'Ordner öffnen',
    openLog: 'Protokoll öffnen',
  },
  portal: portalName,
  field: {
    reveal: 'Passwort zeigen',
    conceal: 'Passwort verbergen',
    clear: 'Suche leeren',
  },
  score: {
    /** `Passung 87 %` - the number comes formatted from format.ts. */
    value: (percent: string) => `Passung ${percent}`,
    excluded: 'Ausgeschlossen',
    unscorable: 'Nicht bewertbar',
    pending: 'Wird bewertet',
    none: 'Ohne Passung',
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
      open: 'Offen',
      violation: 'Ausschlussgrund',
      check: 'Zu prüfen',
    } satisfies Record<ReasonKind, string>,
    weight: {
      must: 'Muss',
      nice: 'Kann',
      hard: 'Ausschluss',
      info: 'Hinweis',
    } satisfies Record<ReasonWeight, string>,
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
      failed: 'Abruf fehlgeschlagen',
      unfetchable: 'Nicht abrufbar',
      gone: 'Nicht mehr online',
    } satisfies Record<Exclude<DetailState['kind'], 'ok'>, string>,
    unread: 'Neu',
    pinned: 'Gemerkt',
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
    sort: 'Sortierung',
    sortMatch: 'Beste Passung',
    sortNewest: 'Neueste',
    search: 'Suchen',
    searchLabel: 'Jobs durchsuchen',
    needsMailbox: 'Erst ein Postfach verbinden.',
  },
  run: {
    never: 'Noch kein Abruf',
    newCount: (value: number) => count(value, 'neuer Job', 'neue Jobs'),
    topCount: (value: number) => `${n(value)} mit hoher Passung`,
    step: {
      scan: 'Postfach',
      fetch: 'Details',
      score: 'Bewertung',
      export: 'Dateien',
    } satisfies Record<Step, string>,
    status,
    of: (done: number, total: number) => `${n(done)} von ${n(total)}`,
    resumesIn: (ms: number) => `Weiter in ${formatCountdown(ms)}`,
    pause,
    pausedUntil: (iso: string | null) =>
      iso ? `Pause bis ${formatMoment(iso)}.` : 'Pause bis zum nächsten Abruf.',
    quota: (iso: string) => `Das Limit ist erreicht, weiter ab ${formatMoment(iso)}.`,
    loginNeeded: 'Die Anmeldung ist abgelaufen.',
    kind: {
      fetch: 'Abruf',
      details: 'Details holen',
      rescore: 'Neu bewerten',
      fullMailbox: 'Ganzes Postfach',
    } satisfies Record<RunKindName, string>,
    done: 'Abruf fertig',
    nothingNew: 'Nichts Neues seit dem letzten Abruf.',
    cancelled: 'Der Abruf wurde abgebrochen.',
    failed: 'Der Abruf ist fehlgeschlagen.',
    skipped: (value: number) => `${count(value, 'Job folgt', 'Jobs folgen')} beim nächsten Abruf.`,
    filesFailed: (value: number) =>
      count(value, 'Datei ließ', 'Dateien ließen') + ' sich nicht schreiben.',
    openOverview: 'Übersicht öffnen',
    history: 'Verlauf',
    collapse: 'Einklappen',
    expand: 'Ausklappen',
    alert: (portal: Portal, postings: number) =>
      `Alert-Mail von ${portalName[portal]} mit ${count(postings, 'Job', 'Jobs')}`,
    health: (portal: Portal) => `${portalName[portal]} meldet sich`,
    checkMailbox: 'Postfach prüfen',
  },
  list: {
    label: 'Jobs',
    excluded: (value: number) => `Ausgeschlossen ${n(value)}`,
    emptyNew: 'Keine neuen Jobs.',
    emptyAll: 'Nach dem ersten Abruf stehen die Jobs hier.',
    emptyAfterRun: 'Die Alert-Mails enthielten bisher keine Jobs.',
    emptyFilter: 'Dazu gibt es gerade keine Jobs.',
    noHit: (query: string) => `Keine Jobs zu „${query}“.`,
    showAll: 'Alle zeigen',
    loadFailed: 'Die Liste ließ sich nicht laden.',
    noProfile: 'Ohne Profil gibt es keine Passung.',
    pickProfile: 'Profil wählen',
    filter: {
      high: 'Hohe Passung',
      noDetail: 'Ohne Details',
      excluded: 'Ausgeschlossen',
    },
    clearFilter: 'Filter entfernen',
  },
  reader: {
    mustMet: (met: number, total: number, partial = 0) =>
      `${n(met)} von ${n(total)} Muss-Anforderungen erfüllt` +
      (partial > 0 ? `, ${n(partial)} teilweise` : ''),
    noMust: 'Keine Muss-Anforderungen erkannt',
    criteria: 'Ausschlusskriterien',
    contract,
    contractLabel: 'Vertragsart',
    criterion: criteria,
    criterionState: {
      met: 'Erfüllt',
      violated: 'Verletzt',
      unknown: 'Zu prüfen',
      unset: 'Nicht gesetzt',
    } satisfies Record<CriterionState, string>,
    note,
    open: 'Anzeige öffnen',
    pin: 'Merken',
    mail: 'Alert-Mail öffnen',
    fetchDetails: 'Details holen',
    why: 'Warum',
    met: 'Erfüllt',
    missing: 'Offen',
    check: 'Zu prüfen',
    violations: 'Verstöße',
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
    high: 'Hohe Passung',
    noDetail: 'Ohne Details',
    excluded: 'Ausgeschlossen',
    issues: 'Offene Punkte',
    lastRun: 'Letzter Abruf',
    newOn: (portal: string, value: number) => `${n(value)} neu auf ${portal}`,
    nothingNew: 'Keine neuen Jobs',
    emptyAlert: (portal: Portal) =>
      `Eine Alert-Mail von ${portalName[portal]} enthielt keine Jobs.`,
    openGmail: 'In Gmail öffnen',
  },
  health: {
    ok: 'Bereit',
    paused: 'Pausiert',
    quotaReached: 'Limit erreicht',
    layoutSuspect: 'Auffällig',
    loginRequired: 'Anmeldung nötig',
    layoutText: (mails: number) =>
      `${count(mails, 'Alert-Mail', 'Alert-Mails')} ohne erkannte Jobs, das Mail-Format hat sich vielleicht geändert.`,
    loginText: 'Die Anmeldung ist abgelaufen.',
  },
  profile: {
    none: 'Noch kein Profil',
    noneText: 'Gegen das Profil wird jeder Job geprüft.',
    pick: 'Profil wählen',
    template: 'Vorlage speichern',
    templateSaved: 'Die Vorlage liegt im Arbeitsordner.',
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
    rescoring: (value: number) => `${n(value)} Jobs werden neu bewertet.`,
    rescored: 'Neu bewertet.',
    parseError: 'Das Profil ist nicht mehr lesbar.',
    understood: 'Das hat die App verstanden',
    competences: 'Kompetenzen',
    more: (value: number) => `+${n(value)}`,
    criteria: 'Ausschlusskriterien',
    warnings: 'Hinweise',
    warning,
    background: 'Werdegang',
    years: (value: number) => `${n(value)} Jahre Berufserfahrung`,
    packs: 'Fachgebiete',
    pack: {
      finance: 'Finanzen',
      sap: 'SAP',
      itProject: 'IT-Projekte',
    } as Record<string, string>,
    notYet: 'Die Auswertung folgt nach dem nächsten Start.',
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
    connect: 'Verbinden',
    removeMailbox: 'Postfach entfernen?',
    removeMailboxText: 'Das App-Passwort wird gelöscht, die Jobs bleiben.',
    autoFetch: 'Beim Start abrufen',
    autoFetchHint: 'Wenn der letzte Abruf mehr als sechs Stunden her ist.',
    active: 'Aktiv',
    details: 'Details holen',
    needsActive: 'Erst das Portal aktivieren.',
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
    quota: (used: number, cap: number) => `Heute ${n(used)} von ${n(cap)} Abrufen`,
    signedIn: 'Angemeldet',
    signedOut: 'Nicht angemeldet',
    signIn: 'Anmelden',
    signOut: 'Abmelden',
    openPortal: 'Im Browser öffnen',
    signInWaiting: 'Das Anmeldefenster ist offen.',
    workspace: 'Arbeitsordner',
    workspaceDefault: 'Standard',
    excel: 'Excel-Übersicht',
    excelShow: 'Im Ordner zeigen',
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
    resetDone: 'Die App wurde zurückgesetzt.',
    resetFailed: (value: number) =>
      `${count(value, 'Datei ließ', 'Dateien ließen')} sich nicht löschen.`,
    running: 'Ein Abruf läuft gerade.',
    dryRun: 'Probelauf, es werden keine Daten verändert.',
  },
  firstRun: {
    benefit: 'Die App liest die Job-Alerts aus Gmail und zeigt, welche Jobs zum Profil passen.',
    privacy: 'Alles bleibt auf diesem Rechner.',
    steps: 'Erste Schritte',
    mailbox: 'Postfach',
    profile: 'Profil',
    profileOr: 'Oder erst eine Vorlage speichern und ausfüllen.',
    fetch: 'Erster Abruf',
    fetchHint: 'Das dauert ein paar Minuten.',
  },
  shell: {
    loadFailed: 'Die App konnte ihre Daten nicht laden.',
    last: (iso: string) => `Zuletzt ${formatMoment(iso)}`,
    showRun: 'Abruf anzeigen',
    runFailed: 'Abruf fehlgeschlagen',
  },
  toast: {
    saved: 'Gespeichert.',
    copied: 'Kopiert.',
    runDone: (value: number) =>
      value === 0
        ? 'Abruf fertig, nichts Neues.'
        : `Abruf fertig, ${count(value, 'neuer Job', 'neue Jobs')}.`,
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
