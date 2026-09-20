// Die einzige Stelle mit Zugriff auf Tauri. Alles andere kennt nur diese Funktionen –
// so lässt sich die Oberfläche im Prüfstand ohne Rust betreiben (ein Vertragstest hält das fest).

const tauri = window.__TAURI__;

/** Fehler, die der Nutzer versteht: Das Backend nennt eine Art, sonst ist es ein Fehler von uns. */
const EXPECTED = new Set(['busy', 'invalid', 'notFound', 'fileLocked', 'mailAuth', 'paused', 'dryRun']);

class CommandError extends Error {
  constructor(kind, message) {
    super(message);
    this.kind = kind;
  }
}

function fail(error) {
  const kind = error?.kind;
  if (EXPECTED.has(kind)) return new CommandError(kind, error.message);
  const text = error?.message ?? String(error ?? 'unbekannter Fehler');
  return new CommandError(kind ?? 'unknown', text);
}

async function call(name, args = undefined) {
  try {
    return await tauri.core.invoke(name, args);
  } catch (error) {
    throw fail(error);
  }
}

/** Ereignisse eines Laufs kommen über einen Kanal – ein Rückruf je Ereignis. */
function channel(onEvent) {
  const ch = new tauri.core.Channel();
  ch.onmessage = onEvent;
  return ch;
}

export const api = {
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
  clearTxtFiles: () => call('clear_txt_files'),
  openTarget: (target) => call('open_target', { target }),
  resetAll: () => call('reset_all'),
  portalLogin: (portal) => call('portal_login', { portal }),
  portalLogout: (portal) => call('portal_logout', { portal }),
};

export const appWindow = {
  minimize: () => tauri.window.getCurrentWindow().minimize(),
  toggleMaximize: () => tauri.window.getCurrentWindow().toggleMaximize(),
  close: () => tauri.window.getCurrentWindow().close(),
  isMaximized: () => tauri.window.getCurrentWindow().isMaximized(),
  /** Die App beendet sich – die Seite zeigt nur noch an, dass aufgeräumt wird. */
  onClosing: (handler) => tauri.event.listen('closing', handler),
};

// Fehler der Seite landen im Protokoll der App, nicht in einer Konsole, die niemand sieht.
const report = (message, source, line) => {
  tauri.core.invoke('report_ui_error', { message, source, line }).catch(() => {});
};
addEventListener('error', (event) => report(String(event.message), event.filename, event.lineno));
addEventListener('unhandledrejection', (event) => report(String(event.reason), null, null));
