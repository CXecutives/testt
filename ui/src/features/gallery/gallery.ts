// Data and sample texts of the gallery (development and harness only; never in the release
// build). The German sample texts are allowed here, like in de.ts.

import type { JobView } from '$lib/ipc/types';

export const text = {
  title: 'Galerie',
  intro: 'Alle Tokens und Bausteine auf einer Seite, gerechnet aus tokens.css.',
  sections: {
    colours: 'Farben',
    type: 'Schrift',
    spacing: 'Abstände',
    radii: 'Radien',
    shadows: 'Schatten',
    gradients: 'Verläufe',
    motion: 'Bewegung',
    icons: 'Symbole',
    buttons: 'Knöpfe',
    navigation: 'Navigation und Fenster',
    tiles: 'Kacheln',
    empty: 'Leerzustand',
    activity: 'Aktivität',
  },
  contrast: {
    against: 'Kontrast',
    pass: 'AA',
    large: 'AA groß',
    fail: 'unter AA',
    exception: 'Ausnahme',
  },
  sample: 'Interim CFO für ein Familienunternehmen in Hamburg',
  motion: {
    play: 'Abspielen',
    reduced: 'Reduzierte Bewegung ist an.',
    full: 'Volle Bewegung ist an.',
    item: 'Job',
    count: 'Passung',
    micro: 'Berühren und drücken',
    fetch: 'Abrufen',
    open: 'Anzeige öffnen',
    save: 'Vorlage speichern',
    back: 'Zurück',
    remove: 'Entfernen',
    pin: 'Merken',
    best: 'Beste Passung zuerst',
    newest: 'Neueste zuerst',
    link: 'App-Passwort erstellen',
    more: 'Einer mehr',
    less: 'Einer weniger',
    again: 'Noch einmal',
    passage: 'Controlling im Konzern',
    password: 'App-Passwort',
    shake: 'Falsches Passwort',
  },
  buttons: {
    fetch: 'Abrufen',
    cancel: 'Abbrechen',
    save: 'Speichern',
    remove: 'Entfernen',
    pin: 'Merken',
    open: 'Öffnen',
    busy: 'Erst nach dem laufenden Abruf möglich.',
    rest: 'Ruhe',
    icon: 'Mit Symbol',
    iconOnly: 'Nur Symbol',
    loading: 'Lädt',
    disabled: 'Gesperrt',
  },
  empty: {
    heading: 'Noch keine Jobs',
    text: 'Der erste Abruf liest die Alert-Mails der letzten 30 Tage.',
    action: 'Abrufen',
    secondary: 'Postfach prüfen',
  },
  navigation: {
    tabs: ['Jobs', 'Profil', 'Einstellungen'],
    toast: 'Toast zeigen',
    toastText: 'Gespeichert.',
    status: 'Zuletzt 08:30',
    running: 'Holt Details',
  },
  surfaces: {
    cards: 'Karten',
    plain: 'Eine ruhige Fläche für Inhalte.',
    interactive: 'Eine Karte, die sich anklicken lässt.',
    tinted: 'Eine hervorgehobene Fläche.',
    badges: 'Abzeichen',
    badge: 'Hinweis',
    loading: 'Platzhalter und Fortschritt',
    meter: 'Fortschritt des Abrufs',
    split: 'Liste und Anzeige',
    list: 'Liste',
    reader: 'Anzeige',
  },
  inputs: {
    heading: 'Eingaben',
    settings: 'Einstellungen',
    toggle: 'Beim Start abrufen',
    toggleHint: 'Ruft neue Alert-Mails ab, wenn der letzte Abruf über sechs Stunden her ist.',
    locked: 'Anmelden',
    lockedReason: 'Erst nach dem laufenden Abruf möglich.',
    risk: 'Konto betroffen',
    facet: 'Ansicht',
    facets: ['Neu', 'Alle'],
    views: ['Neu', 'Alle', 'Gemerkt', 'Bewerbungen'],
    sort: 'Sortierung',
    sorts: ['Beste Passung', 'Neueste', 'Portal'],
    address: 'Postfach',
    addressHint: 'Die Gmail-Adresse, an die die Alert-Mails gehen.',
    password: 'App-Passwort',
    passwordError: 'Das App-Passwort hat 16 Zeichen.',
    createPassword: 'App-Passwort erstellen',
    search: 'Jobs durchsuchen',
    disclosure: 'Mehr zu diesem Portal',
    disclosureText: 'Die App liest nur Links aus den eigenen Alert-Mails.',
    chips: 'Werkzeuge und Methoden',
    chipsHint: 'Enter fügt hinzu, eine Liste mit Kommas wird aufgeteilt.',
    chipValues: ['SAP S/4HANA', 'LucaNet', 'Power BI'],
    chipsEmpty: 'Branchen',
    chipsPlaceholder: 'Maschinenbau',
    chipsShown: 'Schwerpunkte',
    chipsShownValues: ['Controlling', 'Konzernrechnungslegung nach IFRS'],
    area: 'Antwort von Claude einfügen',
  },
  feedback: {
    rings: 'Passung',
    stats: 'Kennzahlen',
    statNew: 'Neue Jobs',
    statHigh: 'Hohe Passung',
    statIssues: 'Offene Punkte',
    statHint: 'Seit dem letzten Abruf',
    statPinned: 'Gemerkt',
    statFilter: 'Ohne Details',
    countMore: 'Einer mehr',
    countLess: 'Einer weniger',
    notices: 'Hinweise',
    noticeHeading: 'Portal pausiert',
    noticeText: 'freelance.de meldet zu viele Anfragen und ist bis 14:30 pausiert.',
    noticeAction: 'Details',
    dialogs: 'Dialoge',
    confirmOpen: 'Bestätigen',
    dangerOpen: 'Zurücksetzen',
    confirmHeading: 'Profil ersetzen',
    confirmText: 'Das neue Profil gilt ab sofort für alle Jobs.',
    confirmLabel: 'Ersetzen',
    dangerHeading: 'Alles zurücksetzen',
    dangerText: 'Jobs, Einstellungen und Anmeldungen werden gelöscht.',
    dangerLabel: 'Zurücksetzen',
    dangerError: 'Im Probelauf lässt sich nichts zurücksetzen.',
  },
  match: {
    reasons: 'Gründe',
    rows: 'Jobliste',
    shuffle: 'Sortieren',
    replay: 'Neu einblenden',
    reasonLabels: {
      met: 'Controlling mit SAP S/4HANA',
      partial: 'Konzernabschluss nach IFRS',
      open: 'Treasury-Erfahrung',
      violation: 'Arbeitnehmerüberlassung',
      check: 'Start in sechs Wochen',
    },
    evidence: '„Controlling im Konzern“ passt zu „Konzerncontrolling“ im Profil.',
  },
} as const;

/* --------------------------------------------------------------- sample jobs */

const HOUR = 3_600_000;

function sample(
  now: Date,
  id: string,
  title: string,
  company: string,
  location: string,
  hoursAgo: number,
  extra: Partial<JobView>,
): JobView {
  const at = new Date(now.getTime() - hoursAgo * HOUR).toISOString();
  return {
    key: { portal: 'freelancermap', id },
    portal: 'freelancermap',
    title,
    company,
    location,
    workMode: 'hybrid',
    mailDate: at,
    firstSeenAt: at,
    unread: false,
    pinned: false,
    detail: { kind: 'ok' },
    short: false,
    match: null,
    alsoOn: [],
    appStatus: null,
    statusAt: null,
    followUpOn: null,
    archived: false,
    overridden: false,
    ...extra,
  };
}

/** A job whose ad states no key facts. */
const NO_FACTS = {
  rate: null,
  hourly: null,
  currency: null,
  rateOpen: null,
  start: null,
  months: null,
  remoteFrom: null,
  remoteTo: null,
  contract: null,
};

const scored = (score: number, top: string): JobView['match'] => ({
  score,
  band: score >= 80 ? 'high' : score >= 40 ? 'mid' : 'low',
  status: 'scored',
  note: null,
  mustMet: 3,
  mustTotal: 4,
  top: [top],
  facts: NO_FACTS,
});

/** Jobs in every state a row can show. */
export function sampleJobs(now: Date): JobView[] {
  return [
    sample(
      now,
      '1001',
      'Interim CFO (m/w/d) für Familienunternehmen',
      'Hanseatic Holding GmbH',
      'Hamburg',
      2,
      {
        unread: true,
        pinned: true,
        appStatus: 'saved',
        match: scored(91, 'Interim-Management im Mittelstand'),
        alsoOn: ['linkedin'],
      },
    ),
    sample(now, '1002', 'Controlling Lead Transformation', 'Nordlicht Energie AG', 'Bremen', 5, {
      portal: 'linkedin',
      key: { portal: 'linkedin', id: '1002' },
      unread: true,
      workMode: 'remote',
      appStatus: 'interview',
      match: scored(64, 'Controlling mit SAP S/4HANA'),
    }),
    sample(now, '1003', 'Kaufmännische Leitung Projektgeschäft', 'Werft 7 GmbH', 'Kiel', 30, {
      portal: 'freelance',
      key: { portal: 'freelance', id: '1003' },
      detail: { kind: 'teaser' },
      match: scored(47, 'Projektcontrolling'),
    }),
    sample(
      now,
      '1004',
      'SAP FI Berater für die Migration der Konzernbuchhaltung auf S/4HANA mit weltweitem Rollout in vierzehn Ländern',
      'Datenwerk Süd',
      'München',
      52,
      {
        workMode: 'onsite',
        match: scored(28, 'SAP FI im Konzern'),
      },
    ),
    sample(now, '1005', 'Finance Manager Shared Service', 'Contoso Services', 'Leipzig', 300, {
      match: null,
      detail: { kind: 'failed', attempts: 3, retryAt: null },
    }),
    sample(
      now,
      '1006',
      'Buchhalter über Personaldienstleister',
      'Musterpersonal GmbH',
      'Berlin',
      200,
      {
        workMode: null,
        unread: true,
        match: {
          score: 55,
          band: 'mid',
          status: 'excluded',
          note: null,
          mustMet: 2,
          mustTotal: 4,
          top: ['Controlling im Konzern'],
          facts: NO_FACTS,
        },
      },
    ),
  ];
}

/** How a colour token is checked for contrast. */
export type ContrastRole = 'text' | 'fill' | 'surface' | 'decor';

export interface ColourToken {
  name: string;
  role: ContrastRole;
  /** Documented exception (the light cxpertise coral of the primary button, user decision). */
  exception?: boolean;
}

export const colourGroups: readonly { title: string; tokens: readonly ColourToken[] }[] = [
  {
    title: 'Flächen',
    tokens: [
      { name: 'bg', role: 'surface' },
      { name: 'surface', role: 'surface' },
      { name: 'surface-muted', role: 'surface' },
      { name: 'surface-hover', role: 'surface' },
      { name: 'surface-press', role: 'surface' },
      { name: 'surface-selected', role: 'surface' },
      { name: 'surface-inverse', role: 'decor' },
      { name: 'scrim', role: 'decor' },
    ],
  },
  {
    title: 'Text',
    tokens: [
      { name: 'text', role: 'text' },
      { name: 'text-muted', role: 'text' },
      { name: 'text-subtle', role: 'text' },
      { name: 'text-heading', role: 'text' },
      { name: 'accent-text', role: 'text' },
    ],
  },
  {
    title: 'Linien',
    tokens: [
      { name: 'border', role: 'decor' },
      { name: 'border-strong', role: 'decor' },
      { name: 'border-input', role: 'decor' },
      { name: 'border-accent', role: 'decor' },
    ],
  },
  {
    title: 'Koralle, handeln, neu und hier',
    tokens: [
      { name: 'accent', role: 'decor' },
      { name: 'accent-soft', role: 'surface' },
      { name: 'primary', role: 'fill', exception: true },
      { name: 'primary-hover', role: 'fill', exception: true },
      { name: 'primary-active', role: 'fill', exception: true },
      { name: 'toggle-on', role: 'decor' },
      { name: 'unread', role: 'decor' },
      { name: 'surface-selected', role: 'surface' },
      { name: 'surface-selected-hover', role: 'surface' },
      { name: 'surface-selected-inactive', role: 'surface' },
      { name: 'selection-bar', role: 'decor' },
      { name: 'ring-track-selected', role: 'decor' },
      { name: 'nav-active-fg', role: 'text' },
      { name: 'nav-active-icon', role: 'decor' },
      { name: 'count-soft-bg', role: 'surface' },
      { name: 'count-soft-fg', role: 'text' },
    ],
  },
  {
    title: 'Navy, Daten und Struktur',
    tokens: [
      { name: 'count-bg', role: 'fill' },
      { name: 'icon-accent', role: 'text' },
      { name: 'active-surface', role: 'surface' },
      { name: 'active-edge', role: 'decor' },
      { name: 'active-text', role: 'text' },
      { name: 'border-navy', role: 'decor' },
      { name: 'text-label', role: 'text' },
      { name: 'link', role: 'text' },
      { name: 'link-hover', role: 'text' },
      { name: 'meter-fill', role: 'decor' },
      { name: 'meter-track', role: 'surface' },
      { name: 'pressed', role: 'text' },
      { name: 'focus', role: 'decor' },
      { name: 'caret', role: 'decor' },
      { name: 'selection', role: 'surface' },
      { name: 'toast-bar', role: 'decor' },
    ],
  },
  {
    title: 'Status',
    tokens: [
      { name: 'success', role: 'decor' },
      { name: 'success-strong', role: 'text' },
      { name: 'success-soft', role: 'surface' },
      { name: 'warning', role: 'decor' },
      { name: 'warning-strong', role: 'text' },
      { name: 'warning-soft', role: 'surface' },
      { name: 'danger', role: 'decor' },
      { name: 'danger-strong', role: 'fill' },
      { name: 'danger-soft', role: 'surface' },
      { name: 'info', role: 'text' },
      { name: 'info-strong', role: 'text' },
      { name: 'info-soft', role: 'surface' },
      { name: 'meter-warning', role: 'decor' },
    ],
  },
  {
    title: 'Passung',
    tokens: [
      { name: 'score-ring-0', role: 'decor' },
      { name: 'score-ring-1', role: 'decor' },
      { name: 'score-ring-2', role: 'decor' },
      { name: 'score-ring-3', role: 'decor' },
      { name: 'score-ring-4', role: 'decor' },
      { name: 'score-ring-5', role: 'decor' },
      { name: 'score-ring-6', role: 'decor' },
      { name: 'score-ring-7', role: 'decor' },
      { name: 'score-ring-8', role: 'decor' },
      { name: 'score-ring-9', role: 'decor' },
      { name: 'score-digits', role: 'text' },
      { name: 'score-high-ring', role: 'decor' },
      { name: 'score-high-text', role: 'text' },
      { name: 'score-high-surface', role: 'surface' },
      { name: 'score-mid-ring', role: 'decor' },
      { name: 'score-mid-text', role: 'text' },
      { name: 'score-mid-surface', role: 'surface' },
      { name: 'score-low-ring', role: 'decor' },
      { name: 'score-low-text', role: 'text' },
      { name: 'score-low-surface', role: 'surface' },
      { name: 'score-track', role: 'decor' },
      { name: 'score-excluded', role: 'text' },
      { name: 'score-excluded-track', role: 'decor' },
    ],
  },
];

export const typeScale = [
  { name: 'xs', spec: '12/16' },
  { name: 'sm', spec: '13/18' },
  { name: 'md', spec: '15/22' },
  { name: 'body', spec: '15/24' },
  { name: 'lg', spec: '17/24 · 600' },
  { name: 'xl', spec: '20/28 · 600' },
  { name: '2xl', spec: '26/32 · 600' },
  { name: 'display', spec: '34/40 · 600' },
] as const;

export const spacing = [2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64] as const;
export const radii = ['xs', 'sm', 'md', 'lg', 'xl', 'full'] as const;
export const shadows = [
  'sh-xs',
  'sh-thumb',
  'sh-pop',
  'sh-hover',
  'sh-primary',
  'focus-halo',
  'focus-ring',
] as const;
export const gradients = ['grad-shimmer'] as const;
export const durations = ['instant', 'hover', 'fast', 'base', 'slow', 'reveal'] as const;
export const easings = ['standard', 'out', 'in', 'emphasized'] as const;

/* ------------------------------------------------------------------ contrast */

export type Rgba = [number, number, number, number];

export function parseColour(value: string): Rgba | null {
  const match = /rgba?\(([^)]+)\)/.exec(value);
  const parts = match?.[1]
    ?.split(/[\s,/]+/)
    .filter(Boolean)
    .map(Number);
  if (!parts || parts.length < 3) return null;
  const [r = 0, g = 0, b = 0, a = 1] = parts;
  return [r, g, b, a];
}

/** Composite a (possibly transparent) colour over an opaque one. */
export function over(top: Rgba, bottom: Rgba): Rgba {
  const a = top[3];
  return [
    top[0] * a + bottom[0] * (1 - a),
    top[1] * a + bottom[1] * (1 - a),
    top[2] * a + bottom[2] * (1 - a),
    1,
  ];
}

function luminance([r, g, b]: Rgba): number {
  const channel = (c: number): number => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG 2 contrast ratio of two opaque colours. */
export function contrast(a: Rgba, b: Rgba): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}
