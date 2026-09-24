// German UI catalog - the only source of UI text.
//
// Style rules: one fact = one sentence; buttons are an infinitive verb without a period;
// sentences end with a period; no text twice. Glossary: Job · Portal · Passung · Details ·
// Abrufen · Profil · Postfach · Alert-Mail · Übersicht · Ausgeschlossen · Neu · Zu prüfen ·
// Merken.

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
  views: {
    jobs: {
      text: 'Hier erscheinen die Jobs aus den Alert-Mails mit ihrer Passung.',
      action: 'Postfach einrichten',
    },
    profile: {
      text: 'Hier liegt das Profil, gegen das jeder Job geprüft wird.',
      action: 'Profil wählen',
    },
    settings: {
      text: 'Hier liegen Postfach, Abruf, Portale und Dateien.',
      action: 'Zu den Jobs',
    },
    firstRun: {
      heading: 'Willkommen',
      text: 'Drei Schritte, dann läuft der erste Abruf.',
      action: 'Loslegen',
    },
  },
  common: {
    loading: 'Wird geladen',
    cancel: 'Abbrechen',
  },
  portal: {
    linkedin: 'LinkedIn',
    freelance: 'freelance.de',
    freelancermap: 'freelancermap',
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
    unscorable: 'Nicht bewertbar',
    pending: 'Wird bewertet',
    none: 'Ohne Passung',
    band: {
      high: 'Hohe Passung',
      mid: 'Mittlere Passung',
      low: 'Geringe Passung',
    },
  },
  reason: {
    kind: {
      met: 'Erfüllt',
      partial: 'Teilweise erfüllt',
      open: 'Offen',
      violation: 'Ausschlussgrund',
      check: 'Zu prüfen',
    },
    weight: {
      must: 'Muss',
      nice: 'Kann',
      hard: 'Ausschluss',
      info: 'Hinweis',
    },
    evidence: 'Beleg im Profil',
  },
  job: {
    workMode: {
      remote: 'Remote',
      hybrid: 'Hybrid',
      onsite: 'Vor Ort',
    },
    /** Badge per DetailState kind (`ok` shows none). */
    detail: {
      pending: 'Details folgen',
      teaser: 'Nur Anriss',
      failed: 'Abruf fehlgeschlagen',
      unfetchable: 'Nicht abrufbar',
      gone: 'Nicht mehr online',
    },
    unread: 'Neu',
    pinned: 'Gemerkt',
    alsoOn: (portals: string) => `Auch auf ${portals}`,
  },
} as const;

export type Catalog = typeof de;
