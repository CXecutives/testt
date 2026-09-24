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
      heading: 'Jobs',
      text: 'Hier erscheinen die Jobs aus den Alert-Mails mit ihrer Passung.',
    },
    profile: {
      heading: 'Profil',
      text: 'Hier liegt das Profil, gegen das jeder Job geprüft wird.',
    },
    settings: {
      heading: 'Einstellungen',
      text: 'Hier liegen Postfach, Abruf, Portale und Dateien.',
    },
    firstRun: {
      heading: 'Willkommen',
      text: 'Drei Schritte, dann läuft der erste Abruf.',
    },
  },
  common: {
    loading: 'Wird geladen',
  },
} as const;

export type Catalog = typeof de;
