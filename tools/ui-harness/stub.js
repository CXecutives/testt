// Nachgebautes Backend für den Prüfstand: dieselben Befehle, dieselben Feldnamen.
// Weicht der echte Vertrag davon ab, fällt es hier zuerst auf.
//
// URL-Schalter: ?jobs=N  Anzahl Jobs · ?dry  Trockenlauf · ?nogmail  kein Postfach
//               ?first   Erststart (keine Jobs, kein Postfach) · ?slow=ms  träges Backend
//               ?platform=macos
(() => {
  const params = new URLSearchParams(location.search);
  const num = (name, fallback) => Number(params.get(name) ?? fallback);
  const first = params.has('first');
  const jobCount = first ? 0 : num('jobs', 24);
  const slow = num('slow', 0);
  const hasGmail = !first && !params.has('nogmail');

  const calls = [];
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));

  const PORTALS = [
    { portal: 'linkedin', label: 'LinkedIn', login: 'none' },
    { portal: 'freelance', label: 'freelance.de', login: 'required' },
    { portal: 'freelancermap', label: 'freelancermap.de', login: 'optional' },
  ];

  const iso = (offsetMinutes) => new Date(Date.now() - offsetMinutes * 60000).toISOString();

  const job = (i) => {
    const portal = PORTALS[i % 3];
    const status = ['ok', 'ok', 'missing', 'ok', 'gone'][i % 5];
    return {
      key: { portal: portal.portal, id: String(100000 + i) },
      portalLabel: portal.label,
      title: `Senior Consultant SAP S/4HANA Finance Transformation Projekt Nr. ${i}`,
      company: `Beispiel Beratung GmbH ${i % 7}`,
      location: ['Frankfurt am Main', 'München', 'Remote', 'Hamburg'][i % 4],
      url: `https://example.org/job/${i}`,
      mailDate: iso(i * 60),
      mailSubject: `Neue Jobs für Sie: SAP Berater und ${i} weitere`,
      gmailUrl: i % 4 ? `https://mail.google.com/mail/u/0/#inbox/${i}` : null,
      firstSeenAt: iso(i * 60),
      firstSeenRun: i < 6 ? 7 : 6,
      status,
      statusText: { ok: 'vorhanden', missing: 'fehlt', gone: 'abgelaufen' }[status],
      short: false,
      closed: false,
      descLen: status === 'ok' ? 2400 : 0,
      descError: status === 'missing' ? null : undefined,
      txtName: status === 'ok' ? `2026_${i}.txt` : null,
    };
  };

  const allJobs = Array.from({ length: jobCount }, (_, i) => job(i));

  const portalView = (p, i) => ({
    portal: p.portal,
    label: p.label,
    enabled: true,
    login: p.login,
    signedIn: p.login === 'none' ? null : p.login === 'required' ? true : false,
    confirmedAt: p.login === 'required' ? iso(120) : null,
    pausedUntil: null,
    pauseKind: null,
    pauseReason: null,
    nextFreeAt: null,
    usedHour: i * 2,
    capHour: 20,
    usedDay: i * 3,
    capDay: 40,
    due: first ? 0 : (i === 2 ? 3 : 0),
    open: first ? 0 : (i === 2 ? 3 : 0),
    failed: 0,
    unfetchable: 0,
    gone: 0,
    ok: 6,
    short: 0,
  });

  const appState = () => ({
    platform: params.get('platform') === 'macos' ? 'macos' : 'windows',
    vaultName: params.get('platform') === 'macos' ? 'Schlüsselbund' : 'Windows-Tresor',
    dryRun: params.has('dry'),
    dataDir: 'C:\\Users\\beispiel\\AppData\\Local\\de.cxecutives.job-alert-monitor',
    settings: { workspace: null, portals: PORTALS.map((p) => p.portal) },
    workspace: 'C:\\Users\\beispiel\\Documents\\Job-Alert-Monitor',
    gmailUser: hasGmail ? 'beispiel@gmail.com' : null,
    gmailError: null,
    profile: first ? null : { path: 'C:\\Users\\beispiel\\Documents\\Job-Alert-Monitor\\profil\\beraterprofil.json', bytes: 4096, savedAt: iso(600), content: 'Schlüssel: name, rollen' },
    profileError: null,
    portals: PORTALS.map(portalView),
    zeroPostingMails: [],
    jobsTotal: jobCount,
    lastScanRun: first ? 0 : 7,
    lastRun: first ? null : {
      run: 7,
      outcome: { kind: 'completed' },
      dryRun: params.has('dry'),
      startedAt: iso(32),
      finishedAt: iso(31),
      scan: { mailsFound: 9, mailsChecked: 9, mailsDefective: 0, alertMails: 3, zeroPostingMails: 0, postingsTotal: 8, new: 6, knownBefore: 2, dupInRun: 0 },
      fetch: { queued: 6, perPortal: { linkedin: { ok: 2, short: 0, closed: 0, gone: 0, failed: 0, skipped: 0, stop: null } } },
      export: { overview: 'JobAlerts.xlsx', backup: null, txtWritten: 6, txtFailedCount: 0, txtFailed: [], error: null },
    },
    resultDir: 'C:\\Users\\beispiel\\Documents\\Job-Alert-Monitor\\auswertung',
    profileDir: 'C:\\Users\\beispiel\\Documents\\Job-Alert-Monitor\\profil',
    txtDir: 'C:\\Users\\beispiel\\Documents\\Job-Alert-Monitor\\auswertung\\beschreibungen_txt',
    txtFiles: first ? 0 : 6,
    resetReport: null,
    running: null,
  });

  /* ---------------------------------------------------------------- Lauf */

  let channel = null;

  async function fakeRun() {
    const send = (event) => channel?.onmessage?.(event);
    send({ type: 'status', text: 'Postfach wird durchsucht' });
    await wait(400);
    send({ type: 'log', level: 'info', text: 'Postfach-Abruf startet.', at: new Date().toISOString() });
    for (let i = 0; i < 3; i += 1) {
      await wait(250);
      send({ type: 'alert', portal: PORTALS[i].portal, subject: `Neue Jobs ${i}`, date: new Date().toISOString(), postings: 2 });
    }
    send({ type: 'status', text: 'Jobdetails werden geholt' });
    for (let i = 1; i <= 6; i += 1) {
      await wait(200);
      send({ type: 'progress', step: 'fetch', done: i, total: 6 });
    }
    send({ type: 'log', level: 'ok', text: 'Jobdetails: 6 geholt.', at: new Date().toISOString() });
    send({
      type: 'finished',
      summary: { ...appState().lastRun, finishedAt: new Date().toISOString() },
    });
  }

  /* ---------------------------------------------------------------- Befehle */

  const handlers = {
    app_state: (args) => { channel = args.channel; return appState(); },
    save_settings: (args) => args.input,
    pick_workspace: () => 'C:\\Users\\beispiel\\Documents\\Anderer Ordner',
    save_gmail_credentials: (args) => args.user,
    delete_gmail_credentials: () => true,
    start_run: (args) => { channel = args.channel; void fakeRun(); },
    cancel_run: () => {},
    list_jobs: (args) => {
      const search = (args.query.search || '').toLowerCase();
      return allJobs
        .filter((j) => (args.query.latestRun ? j.firstSeenRun === 7 : true))
        .filter((j) => !search || `${j.title} ${j.company} ${j.location}`.toLowerCase().includes(search));
    },
    job_detail: (args) => ({
      job: allJobs.find((j) => j.key.id === args.key.id) ?? allJobs[0],
      text: `Aufgaben:\n\n• Leitung des Teilprojekts\n• Abstimmung mit den Fachbereichen\n\nAnforderungen:\n\n• Mehrjährige Erfahrung\n• Sehr gute Deutschkenntnisse\n\n(Beispieltext für Job ${args.key.id}.)`,
    }),
    pick_profile: () => appState().profile,
    remove_profile: () => true,
    rewrite_txt: () => ({ txtWritten: 6, txtFailedCount: 0, txtFailed: [], error: null }),
    clear_txt_files: () => ({ removed: 6, failed: [] }),
    open_target: () => {},
    reset_all: () => {},
    report_ui_error: () => {},
    portal_login: () => true,
    portal_logout: () => true,
  };

  window.__fake = { calls, appState };

  window.__TAURI__ = {
    core: {
      invoke: async (name, args) => {
        calls.push([name, args]);
        const handler = handlers[name];
        if (!handler) throw { kind: 'unknown', message: `Befehl ${name} fehlt im Prüfstand` };
        if (slow) await wait(slow);
        return handler(args ?? {});
      },
      Channel: class { constructor() { this.onmessage = null; } },
    },
    window: {
      getCurrentWindow: () => ({
        minimize: () => calls.push(['window.minimize']),
        toggleMaximize: () => calls.push(['window.toggleMaximize']),
        close: () => calls.push(['window.close']),
        isMaximized: async () => { calls.push(['window.isMaximized']); return false; },
      }),
    },
    event: { listen: async () => () => {} },
  };
})();
