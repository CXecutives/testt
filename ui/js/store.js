// Zustand der Oberfläche an einer Stelle. Wer ihn ändert, sagt es über `emit`;
// wer ihn braucht, hört mit `subscribe` zu. Kein Modul liest den Zustand eines anderen.

export const state = {
  /** Antwort von `app_state` (Einstellungen, Portale, Profil, Gmail, letzter Lauf …). */
  app: null,
  view: 'jobs',
  /** Ein Lauf arbeitet. */
  busy: false,
  cancelling: false,
  /** Text, solange ein Anmeldefenster offen ist (sperrt wie ein Lauf). */
  session: null,
  run: emptyRun(),
  log: [],
  /** Eingabe im Postfach-Formular, bis sie gespeichert wird. */
  gmailForm: { user: '', password: '' },
  filter: 'new',
  search: '',
  /** Gewählter Job als `portal:id`. */
  selection: null,
};

function emptyRun() {
  return {
    kind: null,
    status: '',
    level: '',
    until: null,
    done: 0,
    total: 0,
    /** Portale, die der laufende Lauf übersprungen hat. */
    stops: {},
    /** Schlüssel der in diesem Lauf neu gefundenen Jobs. */
    fresh: [],
    /** Alert-Mails ohne erkannte Jobs (nur während des Laufs). */
    emptyAlerts: [],
    loginWaiting: {},
    summary: null,
  };
}

const handlers = new Map();

export function subscribe(topic, handler) {
  handlers.set(topic, [...(handlers.get(topic) ?? []), handler]);
}

export function emit(topic, payload) {
  for (const handler of handlers.get(topic) ?? []) handler(payload);
}

export function setApp(app) {
  state.app = app;
  emit('app', app);
}

export function setView(view) {
  if (state.view === view) return;
  state.view = view;
  emit('view', view);
}

export function setBusy(busy) {
  state.busy = busy;
  if (!busy) state.cancelling = false;
  emit('busy', busy);
}

export function setCancelling(on) {
  state.cancelling = on;
  emit('busy', state.busy);
}

export function setSession(text) {
  state.session = text;
  emit('busy', state.busy);
}

/** Läuft gerade etwas, das die Oberfläche sperrt? */
export const locked = () => state.busy || Boolean(state.session);

/** Warum ein Knopf gerade gesperrt ist – ein Satz für die ganze Oberfläche. */
export const lockedWhy = () => (locked() ? 'Erst nach dem laufenden Vorgang möglich' : null);

export function resetRun(kind) {
  state.run = emptyRun();
  state.run.kind = kind;
  emit('run', state.run);
}

export function updateRun(patch) {
  Object.assign(state.run, patch);
  emit('run', state.run);
}

export function setFilter(filter) {
  state.filter = filter;
  emit('filter', filter);
}

export function setSearch(search) {
  state.search = search;
  emit('filter', state.filter);
}

export function setSelection(key) {
  state.selection = key;
  emit('selection', key);
}

const LOG_MAX = 400;

export function addLog(level, text, at = new Date()) {
  state.log.push({ level, text, at });
  if (state.log.length > LOG_MAX) state.log.shift();
  emit('log', state.log);
}
