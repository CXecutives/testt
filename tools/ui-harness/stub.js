// Nachgebautes __TAURI__ für den Prüfstand – nichts hier spricht mit einem echten Backend.
// Steuerung aus der Konsole: __fake.delays[cmd] = ms, __fake.fail[cmd] = { kind, message },
// __fake.emit(event) (an den laufenden Lauf), __fake.finish(), __fake.calls (alle Aufrufe).
(() => {
  const params = new URLSearchParams(location.search);
  const N = Number(params.get('jobs') || 40);
  const portals = ['linkedin', 'freelance', 'freelancermap'];
  const labels = { linkedin: 'LinkedIn', freelance: 'freelance.de', freelancermap: 'freelancermap.de' };
  const now = new Date('2026-09-19T10:00:00Z');
  const jobs = [];
  for (let i = 0; i < N; i += 1) {
    const portal = portals[i % 3];
    jobs.push({
      key: { portal, id: String(100000 + i) },
      portalLabel: labels[portal],
      title: `Senior Consultant SAP S/4HANA Finance Transformation Projekt Nr. ${i}`,
      company: `Beispiel Beratung GmbH ${i % 50}`,
      location: 'Frankfurt am Main (Hybrid)',
      url: `https://example.invalid/job/${i}`,
      mailDate: new Date(now - i * 3600e3).toISOString(),
      mailSubject: `Neue Jobs für Sie: SAP Berater und ${i % 7} weitere`,
      gmailUrl: 'https://mail.google.com/mail/u/0/#all/18c0ffee',
      firstSeenAt: new Date(now - i * 3600e3).toISOString(),
      firstSeenRun: i < 20 ? 7 : 3,
      status: i % 4 === 0 ? 'missing' : 'ok',
      statusText: i % 4 === 0 ? 'fehlt' : 'vorhanden',
      short: false,
      closed: false,
      descLen: i % 4 === 0 ? 0 : 2345,
      descError: null,
      txtName: i % 4 === 0 ? null : `job_${i}.txt`,
    });
  }
  const portalView = (portal) => ({
    portal, label: labels[portal], needsAccount: portal === 'freelance', pausedUntil: null, pauseKind: null, pauseReason: null,
    nextFreeAt: null, usedHour: 0, capHour: 20, usedDay: 0, capDay: 40, loginNeeded: false, sessionConfirmedAt: null,
    due: 3, open: 5, failed: 0, unfetchable: 0, gone: 0, ok: 10, short: 0,
  });
  const fake = {
    calls: [],
    running: null,
    gmailUser: params.has('nogmail') ? null : 'user@gmail.com',
    dryRun: params.has('dry'),
    settings: { workspace: null, format: 'xlsx', scope: 'new', portals: ['linkedin', 'freelance', 'freelancermap'], firstRunSeen: true },
    delays: {},
    fail: {},
    hooks: {},
    maximized: false,
    closeListeners: [],
    jobs,
  };
  window.__fake = fake;

  class Channel {
    constructor() { this._on = () => {}; }
    set onmessage(fn) { this._on = fn; }
    get onmessage() { return this._on; }
    send(msg) { this._on(msg); }
  }

  function appStateView() {
    return {
      version: '3.0.0', webviewVersion: '140.0.0.0', dryRun: fake.dryRun, dataDir: 'C:\\Users\\x\\AppData\\Roaming\\Job-Alert-Monitor',
      settings: { ...fake.settings }, workspace: 'C:\\Users\\x\\Documents\\Job-Alert-Monitor', gmailUser: fake.gmailUser, gmailError: null,
      profile: null, profileError: null, portals: portals.map(portalView), zeroPostingMails: [],
      jobsTotal: jobs.length, lastScanRun: 7, lastRun: null, resultDir: 'C:\\Users\\x\\Documents\\Job-Alert-Monitor\\auswertung',
      profileDir: 'C:\\Users\\x\\Documents\\Job-Alert-Monitor\\profil',
      txtDir: 'C:\\Users\\x\\Documents\\Job-Alert-Monitor\\auswertung\\beschreibungen_txt',
      resultDirExists: true, resultFiles: 3, firstRunNotice: params.has('first'), resetReport: null, running: fake.running,
    };
  }

  const summary = (extra = {}) => ({
    type: 'finished', run: 9, outcome: { kind: 'completed' }, dryRun: false, startedAt: now.toISOString(), finishedAt: new Date().toISOString(),
    scope: 'new', scan: { alertMails: 3, postingsTotal: 12, new: 5, knownBefore: 6, dupInRun: 1, mailsDefective: 0 },
    fetch: { queued: 5, perPortal: { linkedin: { ok: 4, short: 0, closed: 0, gone: 0, failed: 1, skipped: 0, stop: null } } },
    export: { overview: 'C:\\Users\\x\\Documents\\Job-Alert-Monitor\\auswertung\\JobAlerts.xlsx', txtWritten: 4, txtFailed: [], error: null, backup: null },
    ...extra,
  });

  const handlers = {
    app_state: (a) => { fake.stateChannel = a.channel; return appStateView(); },
    save_settings: (a) => { Object.assign(fake.settings, a.input); return { ...fake.settings }; },
    list_jobs: (a) => {
      let list = a.query.latestRun ? jobs.filter((j) => j.firstSeenRun === 7) : jobs;
      if (a.query.search) list = list.filter((j) => j.title.includes(a.query.search));
      return JSON.parse(JSON.stringify(list));
    },
    job_detail: (a) => ({ job: jobs.find((j) => j.key.id === a.key.id), text: `Volltext zu ${a.key.id}.\n\nAufgaben:\n• Beratung\n• Umsetzung` }),
    start_run: (a) => {
      if (fake.runActive) throw { kind: 'busy', message: 'Es läuft bereits ein Lauf.' };
      fake.runActive = true;
      fake.runChannel = a.channel;
      return null;
    },
    cancel_run: () => { setTimeout(() => fake.finish({ outcome: { kind: 'cancelled' } }), 300); return null; },
    save_gmail_credentials: (a) => { fake.gmailUser = a.user.toLowerCase(); return fake.gmailUser; },
    delete_gmail_credentials: () => { fake.gmailUser = null; return null; },
    portal_login: () => new Promise((resolve) => { fake.resolveLogin = resolve; }),
    portal_logout: () => true,
    rewrite_txt: () => ({ txtWritten: 12, txtFailed: [], txtFailedCount: 0, error: null }),
    clear_result_files: () => ({ removed: 3, failed: [] }),
    pick_workspace: () => 'C:\\Users\\x\\Documents\\Anderswo',
    pick_profile: () => null,
    remove_profile: () => null,
    reset_all: () => null,
    quit: () => null,
    report_ui_error: (a) => { (fake.reports ||= []).push(a.message); return null; },
    open_target: () => null,
  };

  fake.emit = (event) => fake.runChannel?.send(event);
  fake.finish = (extra) => {
    fake.runActive = false;
    fake.emit(summary(extra));
  };

  async function invoke(cmd, args = {}) {
    fake.calls.push([cmd, args]);
    if (fake.hooks[cmd]) await fake.hooks[cmd](args);
    const d = fake.delays[cmd];
    if (d) await new Promise((r) => setTimeout(r, d));
    if (fake.fail[cmd]) throw fake.fail[cmd];
    const h = handlers[cmd];
    return h ? h(args) : null;
  }

  const win = {
    minimize: async () => { fake.calls.push(['window.minimize']); },
    toggleMaximize: async () => { fake.calls.push(['window.toggleMaximize']); fake.maximized = !fake.maximized; },
    close: async () => { fake.calls.push(['window.close']); for (const fn of fake.closeListeners) fn(); },
    isMaximized: async () => { fake.calls.push(['window.isMaximized']); return fake.maximized; },
  };
  window.__TAURI__ = {
    core: { invoke, Channel },
    window: { getCurrentWindow: () => win },
    event: { listen: async (name, fn) => { if (name === 'close-requested') fake.closeListeners.push(fn); return () => {}; } },
  };
})();
