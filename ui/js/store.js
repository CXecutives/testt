// Ein Zustand für die ganze Oberfläche. Ansichten lesen ihn und zeichnen sich neu, wenn
// sich ihr Thema ändert – sie stoßen sich nie gegenseitig im DOM an.

const LOG_KEEP = 500;

export const state = {
  /** Antwort von `app_state` (Einstellungen, Portale, Profil, Gmail, letzter Lauf …). */
  app: null,
  view: 'alerts',
  /** Läuft ein Lauf? */
  busy: false,
  cancelling: false,
  /** Text, solange das freelance.de-Anmeldefenster für An-/Abmelden offen ist (sperrt wie ein Lauf). */
  session: null,
  run: freshRun(),
  /** Verlauf – höchstens 500 Zeilen; Anzeige und „Verlauf kopieren“ sind identisch. */
  log: [],
  /** Eingaben im Gmail-Formular (bleiben beim Neuzeichnen erhalten). */
  gmailForm: { user: null, password: '' },
};

function freshRun() {
  return {
    kind: null, // 'scan' | 'fetch' | 'job'
    /** Was gerade geschieht; `until`: Ende einer Wartezeit (Countdown). */
    status: '',
    until: null,
    progress: null,
    /** Je Portal [Alert-Mails, Einträge] dieses Laufs. */
    alertCounts: {},
    /** Je Portal der Stopp dieses Laufs { skipped, text }. */
    stops: {},
    /** Alert-Mails ohne Einträge aus diesem Lauf (bis der Zustand neu geladen ist). */
    emptyAlerts: [],
    /** Das Anmeldefenster wartet gerade (LoginNeeded) – je Portal. */
    loginWaiting: {},
    /** Abschluss: Zusammenfassung und Zeile für die Laufleiste. */
    summary: null,
    summaryLine: '',
    summaryLevel: '',
  };
}

const listeners = new Map();

/** Themen: 'app' · 'view' · 'busy' · 'run' · 'log' · 'job' (ein Job) · 'jobs' (Liste neu) · 'navigate'. */
export function subscribe(topic, handler) {
  if (!listeners.has(topic)) listeners.set(topic, new Set());
  listeners.get(topic).add(handler);
}

export function emit(topic, payload) {
  for (const handler of listeners.get(topic) ?? []) {
    try {
      handler(payload);
    } catch (error) {
      // Ein Fehler in einer Ansicht darf die anderen nicht aufhalten.
      queueMicrotask(() => { throw error; });
    }
  }
}

export function setApp(app) {
  state.app = app;
  if (state.gmailForm.user === null) state.gmailForm.user = app.gmailUser ?? '';
  emit('app', app);
}

export function setView(view) {
  state.view = view;
  emit('view', view);
}

export function setBusy(busy) {
  state.busy = busy;
  if (!busy) state.cancelling = false;
  emit('busy');
}

export function setSession(text) {
  state.session = text;
  if (!text) state.cancelling = false;
  emit('busy');
}

/** Gesperrt für alles, was das Backend während eines Laufs oder einer Anmeldung ablehnt. */
export const locked = () => state.busy || Boolean(state.session);

/** Warum ein Knopf gerade gesperrt ist – ein Satz für die ganze Oberfläche. */
export const lockedWhy = () => (locked() ? 'Erst nach dem laufenden Vorgang möglich' : null);

export function setCancelling(cancelling) {
  state.cancelling = cancelling;
  emit('busy');
}

export function resetRun(kind) {
  state.run = freshRun();
  state.run.kind = kind;
  emit('run', state.run);
}

export function updateRun(patch) {
  Object.assign(state.run, patch);
  emit('run', state.run);
}

/** Verlaufszeile (info · ok · warn · error); `at` ist die Zeit der Meldung (Backend) oder jetzt. */
export function addLog(level, text, at = new Date()) {
  const entry = { level, text, at: at instanceof Date ? at : new Date(at) };
  state.log.push(entry);
  const drop = state.log.length - LOG_KEEP;
  if (drop > 0) state.log.splice(0, drop);
  emit('log', { entry, dropped: Math.max(0, drop) });
}
