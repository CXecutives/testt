// Die einzige Stelle mit `window.__TAURI__`: Befehle, Ereigniskanäle, Fenster.
// Jeder Befehl ist ein dünner Aufruf; Fehler kommen als ApiError { kind, message }.

const tauri = window.__TAURI__;
const { invoke, Channel } = tauri.core;

class ApiError extends Error {
  constructor(kind, message) {
    super(message);
    this.kind = kind;
  }
}

/** Fehler, mit denen der Nutzer rechnen muss – alle anderen gehören ins Protokoll. */
const EXPECTED = new Set(['busy', 'invalid', 'secret', 'notFound', 'fileLocked', 'io', 'dryRun', 'paused']);

async function call(command, args = {}) {
  try {
    return await invoke(command, args);
  } catch (error) {
    // Befehlsfehler kommen als { kind, message }, IPC-Fehler (Argumente, Rechte) als Text.
    const failure = error && typeof error === 'object' && 'message' in error
      ? new ApiError(error.kind || 'unknown', error.message)
      : new ApiError('ipc', String(error));
    if (!EXPECTED.has(failure.kind) && command !== 'report_ui_error') {
      report(`${command}: ${failure.kind}: ${failure.message}`);
    }
    throw failure;
  }
}

// Höchstens 20 Berichte je Sitzung – ein Fehler in einer Schleife soll das Protokoll nicht
// fluten. Fehler des Berichtens selbst werden verschluckt.
let reports = 0;

function report(message, source = null, line = null) {
  if (reports >= 20) return;
  reports += 1;
  invoke('report_ui_error', { message: String(message).slice(0, 500), source, line }).catch(() => {});
}

function channel(onEvent) {
  const ch = new Channel();
  ch.onmessage = onEvent;
  return ch;
}

export const api = {
  /** Zustand beim Start und nach Neuladen; hängt die Seite an einen laufenden Lauf an. */
  appState: (onEvent) => call('app_state', { channel: channel(onEvent) }),
  saveSettings: (input) => call('save_settings', { input }),
  pickWorkspace: () => call('pick_workspace'),
  saveGmail: (user, password) => call('save_gmail_credentials', { user, password }),
  deleteGmail: () => call('delete_gmail_credentials'),
  startRun: (request, onEvent) => call('start_run', { request, channel: channel(onEvent) }),
  cancelRun: () => call('cancel_run'),
  listJobs: (query) => call('list_jobs', { query }),
  jobDetail: (key) => call('job_detail', { key }),
  pickProfile: () => call('pick_profile'),
  removeProfile: () => call('remove_profile'),
  rewriteTxt: () => call('rewrite_txt'),
  clearResultFiles: () => call('clear_result_files'),
  openTarget: (target) => call('open_target', { target }),
  resetAll: () => call('reset_all'),
  portalLogin: () => call('portal_login'),
  portalLogout: () => call('portal_logout'),
  /** Die Rückfrage „Beenden?“ ist beantwortet – der Wächter im Backend beginnt von vorn. */
  closeAnswered: () => call('close_answered'),
  quit: () => call('quit'),
};

const current = tauri.window.getCurrentWindow();

export const appWindow = {
  minimize: () => current.minimize(),
  toggleMaximize: () => current.toggleMaximize(),
  close: () => current.close(),
  isMaximized: () => current.isMaximized(),
  /** Das Fenster soll während eines Laufs schließen – die Seite fragt nach. */
  onCloseRequested: (handler) => tauri.event.listen('close-requested', handler),
};

// Fehler der Seite landen im Protokoll der App (nie still verloren).
window.addEventListener('error', (event) => {
  report(event.message, event.filename, event.lineno);
});
window.addEventListener('unhandledrejection', (event) => {
  const reason = event.reason;
  report(reason instanceof Error ? `${reason.name}: ${reason.message}` : String(reason));
});
