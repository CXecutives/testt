# Plan - matching back, new UI, everything else reviewed

Approved 2026-09-24. Priorities: (1) bring matching back, at least equal to the old app and measurably better;
(2) a completely new, consistent, lively UI in the cxpertise.de look; (3) review scraping, architecture, code and
project layout. Windows and macOS as identical as possible. Done = shippable Windows installer + macOS .app/.dmg.

## Decisions (user answers, binding)
| Topic | Decision |
|---|---|
| UI language | German, one catalog `ui/src/lib/i18n/de.ts`, no switcher |
| Frontend | Svelte 5 + Vite + TypeScript, no SvelteKit, no animation library, Lucide icons only |
| Keys | only inside fields/dialogs: Tab/Shift+Tab, Enter = save, Esc = cancel, Ctrl/Cmd+C/V/X/A/Z |
| OS window functions | keep Alt+F4, Cmd+Q/W/M/H, double-click on title bar; no own shortcuts |
| Mac | no Mac available: macOS via GitHub `macos-latest` (real app screenshots, dmg install probe, keychain test) + WebKit locally |
| macOS minimum | 14.0 (Safari 17 baseline, `data_store_identifier` for sessions) |
| Evaluation data | no access to Katharina: local real data + real runs through the app, two realistic invented profiles, blind labels by two independent agents + tie-breaker |
| Embeddings | dropped (user, 2026-09-24): the rule engine covers the measured failures; the `Embedder` seam stays for later |
| AI stage | not in the app; the external `job-matching` skill stays and may be improved (phase 5) |
| Extra criteria | only "permanent position detected" as a check hint |
| Scraping | everything switchable per portal (Active / Fetch details / Sign in), safe defaults, risk badge per switch |
| HTML overview | no full text: title, company, location, portal, link, match, 3 met, 2 open, exclusion reason |
| Extras | Pin (star) + auto fetch on start (> 6 h, switchable); no notifications, no "still open?" checks |
| Logo | no CXpertise company logo; the coral app icon (folder + check) is the brand mark in the title bar |
| Heading colour | warm dark ink (45 7% 17%), not slate; coral is the only accent colour (user chose variant A) |
| Windows caption buttons | flat like the Claude desktop app (user 2026-09-24): 46 x 40, Segoe Fluent glyphs 10 px in ink (muted at rest), warm hover ink/.06, pressed ink/.10, close hsl(4 62% 50%) with a white glyph |
| Sizes | smaller (user 2026-09-24): title bar 40, tabs 14/500 (active 600), controls 28/36/40, toolbar 56, list rows 72; body text stays 15 |
| Layout | like the Claude desktop app (user 2026-09-24): left sidebar 232 px (brand, "Abrufen" as the window's primary, nav with icons and unread count, run status at the bottom), icon rail of 64 px below 1100 px; content gets a 40 px drag strip with the caption buttons; word tabs are gone. Test ids `nav-jobs`, `nav-profile`, `nav-settings`, `view-*` |
| Toasts | allowed for short confirmations whose result is not visible otherwise (saved, copied, files written, run finished): bottom right, at most 3, ~4 s, paused on hover; anything needing action stays inline |
| Cleanup outside | `.notes` archived to `../_archive/TEST-notes`; user deletes `origin/ci-macos` and release `latest`; CI publishes nothing |
| Self-decided | TXT header stays German and byte-identical · primary button brand-near (coral 56 %, label 600) · excluded jobs grey behind a divider, also under "Neu" but not counted · Excel for excluded: domain score, grey row · merge cross-portal duplicates · Smart App Control is off on the dev PC |

## Contracts

### Schema 3 (one migration, chain instead of `1 => SCHEMA_VERSION`)
New nullable `job` columns: `match_score INTEGER`, `match_status TEXT` (scored|excluded|unscorable), `match_note TEXT`
(JSON `{code, params, mustMet, mustTotal, top[<=2]}`, <= 400 B), `match_at`, `match_rev TEXT` (16 hex of sha256 over
ENGINE_VERSION + canonical profile view + model id), `read_at`, `desc_facts TEXT`, `dup_of TEXT` (`portal:id`),
`parser_version INTEGER`, `pinned_at`. `desc_status` gains `teaser`. Frozen fixture `core/tests/fixtures/schema_v2.sql`.
Migration marks everything before the last mailbox run as read. `PRAGMA journal_mode=WAL; synchronous=NORMAL`;
reset deletes `-wal`/`-shm`. No FTS5 (table is WITHOUT ROWID): search stays `LIKE` on the folded `search` column.
Reasons are not stored; `job_detail` recomputes them. Settings JSON: per portal `enabled`, `fetchDetails`,
`loginEnabled` (freelance.de), plus `autoFetchOnStart`.

### IPC v3 (types from Rust via ts-rs; camelCase; `null` instead of missing; backend never sends prose)
Commands: `app_state` · `start_run(RunRequest{kind: fetch | details{keys} | rescore | fullMailbox})` · `cancel_run` ·
`list_jobs(JobQuery{facet: new|all, sort: match|newest, search?, limit, offset}) -> JobPage{jobs, counts{new, all, excluded, high, noDetail}}`
(list and counts from ONE query) · `job_detail(key)` · `mark_read(key) -> bool` · `set_pinned(key, on)` · `pick_profile` ·
`remove_profile` · `save_profile_template` · `save_mailbox` · `remove_mailbox` · `portal_login` · `portal_logout` ·
`pick_workspace` · `rewrite_txt` · `clear_txt` · `open_target({jobUrl|gmail|workspace|excel|overview|logDir})` ·
`save_settings(SettingsPatch)` · `reset_all` · `report_ui_error` (truncated, <= 10/min).
Rust triggers `rescore` itself (after pick/remove profile, at start, after an engine update, if pending > 0; pending = 0
without a usable matcher) and the auto fetch (setting on, mailbox connected, last fetch > 6 h).
Events on channel `run` (struct variants, each < 8 KB): `Progress{step: scan|fetch|score|export, portal?, done, total}` ·
`Status{code, portal?, until?}` · `Alert{portal, subject, date, postings, gmailId}` · `JobUpdated{job}` ·
`PortalHealth{portal, health}` · `LoginNeeded{portal, waiting}` ·
`Finished{summary{perPortal[{portal,new,known,dup,fetched,failed}], score{scored,excluded,unscorable,pending,best}, stops[], emptyAlerts[]}}`.
Types: `JobView{key, portal, title, company, location, workMode, mailDate, firstSeenAt, unread, pinned, detail, match{score, band, status, note, mustMet, mustTotal, top[]}|null, alsoOn[]}` ·
`JobDetail{job, text, url, fetchedAt, mail{subject, gmailUrl}, match{score, status, band, rev, at, summary, reasons[<=40], highlights[<=200], criteria[]}|null}` ·
`Reason{id, kind: met|partial|open|violation|check, weight: must|nice|hard|info, code, label, evidence{profile, path, via, quote}|null, params, ranges[]}` ·
`Highlight{id, start, end (UTF-16), kind, reason}` · `ProfileInfo{fileName, bytes, savedAt, quality: good|thin|empty, understood{competenceCount, competences[], sources[], criteria[], warnings[]}, scoredAt, pending}` ·
`PortalHealth = ok | paused{until, reason} | quotaReached{until} | layoutSuspect{emptyMails, pages} | loginRequired` ·
`AppState{platform, dryRun, firstRun, running, settings, mailbox, profile, portals[{portal, enabled, fetchDetails, login, loginEnabled, signedIn, risk: low|grey|account, health, quota?}], autoFetchOnStart, lastRun, counts, topMatches[<=5], matchPending, dataDir, logDir, resetReport?}` ·
`CommandError{kind, params}`. Traits: `pipeline::score::Matcher{rev, assess}` · `portal::PortalAdapter` · `matching::prescore`.

### Matching engine (`core/src/matching`, pure, synchronous, integer only)
Modules: mod, params, types, lexicon/ (external contract), normalize, profile, sections, requirements, signals,
criteria, ladder, relevance, semantic (feature), score, explain.
- Profile compiled once: core phrases with JSON path, languages + level, degrees, total years, criteria, quality
  (empty => not scored). Profile keys stay German (shared with the skill); English aliases also understood.
- Requirements: sections (V5), 3 stages (V16), UND splits / ODER = alternatives (V6), kinds skill/formal/language/years/soft/frame (V7).
- Match ladder: exact · stem · synonym/bilingual = 1.0 · profile more specific = 1.0 · profile more general = 0.5 ·
  2/3 rule (>= 3 atoms) = 0.5 · semantic = 0.5 (feature only). Generic atoms never count alone.
- Weights: must 1000, soft 250, frame 0; nice by count. `M = sum(w*e*500)/sum(w)`, `K = sum(e*500)/n_nice`,
  `P = must ? (nice ? (3M+K)/4 : M) : K`, evidence `n = sum(w_must) + 500*n_nice`, relevance `R = min(1000, R_lex + T/2)`
  (BM25F-like: title x3, requirements x2, rest x1, k1 1.2, b 0.75, fixed length 2400, static specificity),
  shrinkage `P' = (n*P + k*R)/(n + k)`, k = 2000 (calibrated, frozen per ENGINE_VERSION), `score = round_half_even(P'/10)`.
- Status: excluded if >= 1 decided violation (score kept, ring shows no number) · unscorable if too little text · else scored.
  Band only via `model::band`: >= 80 high, 40-79 mid, < 40 low.
- Order: `(match_status IS 'excluded'), (match_score IS NULL), match_score DESC, first_seen_at DESC, portal, job_id`.
- Hard criteria (no threshold in code, missing key = inactive). Decided only on clear wording, otherwise `check`:
  ANUE (named, not negated, not optional) · country (location field/line/on-site sentence/facts, remote not full) ·
  day rate (EUR, upper bound, hourly x8, no clear permanent role) · availability gap is a check only (`availabilityGap {days}`, never an exclusion; decided after the corpus review) ·
  day-rate fallback `einsatzpraeferenzen.tagessatz_ab` kept (old behaviour) · status precedence excluded > unscorable > scored · permanent position detected = check only.
- Scored at `JobUpdated` (rings appear live) + catch-up `Step::Score` (pages of 250) + `rescore` run.
- `legacy_percent()` reproduces the old path for parity tests only.

### Evaluation
- Synthetic corpus in CI (both OS): `sample_profile.json` (interim finance), `sample_profile_it.json` (SAP/IT PM),
  K01-K40 in TXT contract format, `corpus.json` with bands/status/codes fixed BEFORE building (reviewed by an
  independent agent), `legacy.json` frozen once by `tools/eval/legacy_baseline.py` (old matcher via `git show ca9a2cd^`).
- Private gold set (ignored): real local ads + real runs, blind labels 0-3 by two agents with the skill rubric, third breaks ties.
- CI gates: 35 old tests (one documented deviation: `1.200,50`) · `legacy_percent == legacy.json` · per job
  distance(new) <= distance(old), sum strictly smaller · decided exclusions 0 wrong / 0 missing · same golden digest on
  Windows and macOS · median <= 0.5 ms/job, 2000 jobs <= 3 s incl. SQLite.
- Private gates (reported "preliminary" if n < 60): exclusion precision 100 %, recall >= 80 % · NDCG@10 >= 0.75 and
  >= old + 0.10 · P@5 >= 0.8 and >= old · no grade-3 job below 40 · high-band precision >= 0.8 · Spearman >= 0.55 and > old.
- Prescore order only if AUC >= 0.7. Report in `docs/MATCHING.md`.

### Scraping and sign-in
Per portal: Active · Fetch details (off = zero requests to the portal) · Sign in (freelance.de only, default off).
Risk badges: low (freelancermap) · grey, no account affected (LinkedIn guest, freelance.de guest) · account risk
(freelance.de signed in). Always on: only alert-mail links, `admit` for every request, Retry-After honoured, stop and
pause on 429/999/403/captcha/login wall, never bypass captcha/2FA, never an unasked login window.
S1 PortalHealth + per-portal empty-alert warning · S2 freelance.de teaser · S3 `desc_facts` · S4 PortalAdapter registry,
`join_all` · S5 error kinds, PARSER_VERSION, requeue, Retry-After, constants in `policy.rs` · S6 freelancermap URL forms,
SimHash duplicates · S7 prescore order · S8 two-phase IMAP · S10 UA passed from `platform.rs` · S11 codes, English logs with run id.
Sessions: Windows `data_directory`, macOS `data_store_identifier` + `clear_all_browsing_data`; sign out deletes cookies,
cache, profile dir, marker, then verifies `signedIn=false`.

### UI
- Shell (revised 2026-09-24, see Decisions "Layout"): sidebar with brand, Abrufen, nav Jobs · Profil · Einstellungen
  and the run status; a 40 px drag strip over the content with the Windows caption buttons; macOS traffic lights in
  the sidebar's top left. No menu, no gear icon.
- Jobs: toolbar (Abrufen primary lg / Abbrechen · Neu n | Alle n · Beste Passung | Neueste · search) · left column
  run card + list (72 px rows: ring 40, title with unread dot, meta, reason line, date, status badge only on deviation;
  excluded grey behind divider; duplicates as one row) · reader card 720 px (ring 96 counting up, band word, n of m must,
  hard-criteria strip, reasons met/open/check/violations, hover = tooltip + highlight, click = scroll to passage) ·
  day overview when nothing is selected (3 stat tiles, unread per portal, best 3, pinned, open issues, "Übersicht öffnen").
- Profil: file card (choose, save template, remove), "Das hat die App verstanden" card.
- Einstellungen: Postfach · Abruf (auto fetch) · Portale (switches with risk badges, health, quota only >= 80 %) ·
  Dateien · Wartung. First run: full page with three real, self-ticking steps.
- All states per screen (first use, no profile, empty, loading, run, nothing new, no search hit, errors, offline,
  portal paused, no details, teaser, unscorable, excluded). Feedback where the action happened; no toasts.
- Tokens (`tokens.css`, `:root`, light only, `color-scheme: light`): palette from the brief (coral 13 73% 63%, slate
  212 34% 37%, cream 32 33% 96%, ink 45 7% 17%, ...) plus shades (coral-800 13 62% 45% for text, *-strong/*-soft for
  status, info), semantic tokens only in components, score colours (high 152 50% 31%, mid coral-800, low 30 4% 42%),
  gradients coral-only (coral-glow -> coral-variant; NO coral -> slate, user 2026-09-24), card/wash/shimmer; brand mark = the real coral app icon (folder + check) as SVG, warm shadows incl. elegant and glow from the brief, radii 6/8/10/12/16,
  4 px spacing, controls 28/36/40, type 12/13/14 (tabs)/15/15/17/20/26/34 (UI standard 15/22), motion 80/150/220/320/700/600/1400 ms,
  stagger 30 ms, four easings.
- 23 components (Button primary|secondary|ghost|danger x sm|md|lg, Icon, IconTile, Card, Badge, Segmented, Toggle,
  TextField+Field, NavTabs, ScoreRing, Meter, Skeleton, Spinner, Notice, EmptyState, StatTile, Dialog, Tooltip,
  Disclosure, SettingRow, ListRow/JobRow, ReasonItem, WindowControls; since phase 3 SideNav instead of NavTabs, and
  Toast). Not: select, checkbox, radio, context menu.
- Motion: only transform/opacity (colour on hover); shadows/glow on `::after` via opacity; whole-pixel end values;
  <= 10 staggered, <= 10 rings animating, FLIP <= 100 rows else cross-fade; reduced motion via `motion.ts`.
- Consistency: stylelint (no hex/named colours, no colour functions/units outside tokens, strict values, allowed
  transition properties, keyframes only in motion.css) · ESLint (no inline styles, raw elements only in components,
  restricted imports, no title attribute, no empty catch, listeners only in input.ts) · Rust architecture tests ·
  gallery · Playwright screenshots in Chromium + WebKit against `vite preview` with production CSP · per-screen audit.
- Input policy: prevent contextmenu everywhere, non-left buttons, dblclick outside drag area, dragstart, selection
  outside fields, keys outside fields, Ctrl/Cmd+wheel, pinch. Native: WebView2 switches, macOS minimal menu,
  `accept_first_mouse`, no link preview, devtools off in release, navigation guard, window shown after first load.

### Platforms (documented differences only)
Window frame (own caption buttons vs. traffic lights) · menu (none vs. minimal App/Edit/Window) · font smoothing on
macOS · keychain vs. credential manager (same code) · session storage API · reveal in folder (`explorer /select` vs.
`open -R`) · per-OS user agent. Build target Safari 17; forbidden: View Transitions, `@starting-style`,
`scrollbar-gutter`, `content-visibility`. Windows: NSIS currentUser, German installer, downloadBootstrapper.
macOS: universal, ad-hoc signed, minimum 14.0.

## Phases (tick as you go)

### Phase 0 - rescue, clean up, context
- [x] Cherry-pick the bare-URL title fix (c9a78ba)
- [x] Rescue matching corpus to `core/tests/fixtures/matching/` (`sample_profile.json`, `corpus/K01-K30.txt`)
- [x] Save schema-3 draft, stash and CI v5 commit as patches (scratch), remove 4 worktrees, stash, old branches
- [x] Archive `.notes` outside the repo, `cargo clean`, remove `__pycache__`, English `.gitignore`
- [x] `CLAUDE.md` and this file
- Done when: worktree list = main, stash empty, branches = main + legacy-python, `cargo test --workspace` green.

### Phase 1 - contracts and foundation (parallel tracks)
- [x] 1a Contract (merged ab97def; deviations: DetailState.failed={attempts,retryAt}, paused.until nullable, extra OpenTarget variants, app_state(channel) + RunSnapshot; PortalAdapter/prescore still open): IPC v3 in `view.rs` + ts-rs; codes for notices/errors/status; RunEvent v3;
      `RunRequest.kind`; new command signatures; `Matcher` trait with empty score step; `PortalAdapter` trait +
      `prescore` signature; `PortalHealth`; split `store.rs` into `store/{mod,schema,jobs,matches}.rs`; schema-3 column
      list; prose out of core; split `commands.rs`; `contract.rs` reads `.ts`.
      Done when: types generated and `git diff` clean, `contract.rs` green.
- [ ] 1b UI foundation: package.json, Vite, Svelte, TS; tokens/base/motion css, motion.ts, input.ts, api.ts, i18n;
      shell (title bar, tabs, caption buttons); gallery with token boards; Playwright against `vite preview` with
      production CSP; lint setup; new `ui_contract.rs`; old `ui/*` removed.
      Done when: `npm run check` green; a planted hex value, px value and raw `<button>` each turn lint red.
- [x] 1c Platform and delivery (merged 92669ef; macOS parts verified only by CI): tauri.conf (withGlobalTauri false, freezePrototype true, frontendDist `../ui/dist`,
      beforeBuildCommand, hidden window + light theme), `tauri.windows.conf.json` (NSIS), macOS minimum 14.0,
      `platform.rs`, smoke on `data-testid`, `rust-toolchain.toml`, English CI with `tauri build` on both OS,
      macOS dmg install probe with screenshot, no public release.
      Done when: release app starts on Windows and in macOS CI without CSP violation; CI ships setup.exe + dmg.
- [ ] 1d Evaluation base + port: corpus to K40 + second profile, `corpus.json`, `legacy_baseline.py` -> `legacy.json`
      FIRST; `eval.rs`; port "v3-equal" with `legacy_percent`; `matching_legacy.rs` (35 tests).
      Done when: fidelity 31/18/11/7/6/0 reproduced, 35 old tests green (1 documented deviation),
      `legacy_percent == legacy.json` for all K, clippy clean.

### Phase 2 - core work (parallel, disjoint files; contracts frozen)
- [ ] 2A Matching better: V5, V6, V3, V2, V1, V4, V15, V7, V9, V8, V16, V10-V13, decided/check model, V17, V18, V19,
      explain.rs, prescore - one commit each with corpus guard; calibrate, freeze.
- [ ] 2B Store, runs, export (done with 1a except: wiring the real engine as LocalMatcher, profile quality/understood, topMatches, alsoOn, freelance.de guest teaser): schema 3 + chain + WAL; save_match(es), mark_read, set_pinned, job_page; settings
      (portal switches, autoFetchOnStart); LocalMatcher, scoring at JobUpdated + catch-up, Rust-triggered rescore and
      auto fetch; profile summary + template; Excel column + grey header, mail address out of info sheet; HTML
      overview; TXT byte tests; demo with high/mid/low/excluded.
- [ ] 2C Components: all 23 with variants, states, motion; complete gallery; baselines.
- [ ] 2D Scraping and sign-in (session delete, macOS data store, no unasked sign-in window, keychain test already merged with 1c): S1-S11; switches honoured in the fetch path; optional sign-in with risk note; delete
      session per portal (macOS `data_store_identifier`); keychain test on macOS; dead code list.
      Done when: 26 fetch tests + new (4th test portal via registry only, health, teaser, Retry-After, requeue, slug
      URL = same id, duplicate group, IMAP loads candidates only, details off => zero portal requests, sign-out
      verified on both OS).
- [ ] Integration (integrator, serial): wire A + B + D into commands and view.

### Phase 3 - screens and core workflow (two UI agents)
- [x] Jobs (toolbar, run card, list with progressive rendering, reader with reasons and highlights, day overview)
- [x] Shell, Profil, Einstellungen, first run; all states; texts only from `de.ts`; `mark_read` only on a real click
- [ ] >= 30 harness scenarios in Chromium + WebKit; screenshot baselines; smoke probe of the real app
- Done when: core workflow works in both engines and the real app; every view in every state is captured; 0 lint
  exceptions; all 33 audit findings of the old UI are resolved. Send screenshots (Windows + macOS CI) to the user.

### Phase 4 - English sweep (parallel to phase 3)
- [ ] Translate remaining comments, logs, errors, asserts, CI, hook, toml; new English README; `language.rs` with allowlist
- [ ] Finish the dead-code list

### Phase 5 - verification, measurement, audit, skill
- [ ] Real runs through the app (gold set within limits and switches), blind labels, old/new report in `docs/MATCHING.md`
- [ ] Live canary per portal (one counted page via `admit`)
- [ ] Windows installer + first run + screenshots; macOS CI screenshots + dmg probe + WebKit scenarios
- [ ] Performance (start time, long tasks at 2000 jobs), contrast
- [ ] Consistency audit per screen (checklist below) and fixes; one adversarial review workflow over the whole diff
- [ ] Skill `job-matching`: back up original to `../_archive/job-matching-skill-original`, check whether the folder
      is managed, fix hard-coded foreign path, use the app's match as pre-screening, align rubric wording, test, report diff

### Phase 6 - delivery
- [ ] Final CI builds (artifacts only), first-start guide (SmartScreen, Gatekeeper, keychain), close this plan, hand over

## Consistency audit per screen
For each of Jobs, Reader, Day overview, Profil, Einstellungen, First run, dialogs:
- [ ] tokens only · 4 px grid, shared edges aligned · <= 1 primary, button variants by rule · icon sizes by context
- [ ] hover/active/focus/disabled everywhere · all screen states present · glossary, no sentence twice · tabular numbers
- [ ] motion only via tokens, reduced motion checked · AA contrast (documented exception: primary button)
- [ ] Windows and macOS screenshots side by side: only the documented differences

## Glossary (UI)
Job · Portal · Passung · Details · Abrufen · Profil · Postfach · Alert-Mail · Übersicht · Ausgeschlossen · Neu (= unread) ·
Zu prüfen · Merken. Checked for the UI catalog and the Rust export texts.

## Budget and models
The user's usage limit is tight: work token-efficiently without lowering quality - targeted reads, focused test runs,
no sub-agents inside tracks, screenshots only at milestones, commit every finished step. Models (no Haiku):
- **Opus** for everything that shapes the product or the measurement: matching engine, contract, store/pipeline,
  scraping/sign-in logic, UI foundation, components and screens, integration and merges, corpus band review,
  gold-set labelling, skill improvement, consistency audit and the final review.
- **Sonnet** for simple, fully verifiable work: phase 4 comment/log translation (checked by language.rs, build and
  tests), README and first-start guide from finished facts, collecting CI artifacts and screenshots, routine cleanup.
