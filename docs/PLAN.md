# Plan - matching back, new UI, everything else reviewed

Approved 2026-09-24. Priorities: (1) bring matching back, at least equal to the old app and measurably better;
(2) a completely new, consistent, lively UI in the cxpertise.de look; (3) review scraping, architecture, code and
project layout. Windows and macOS as identical as possible. Done = shippable Windows installer + macOS .app/.dmg.

## Decisions (user answers, binding)
| Topic | Decision |
|---|---|
| UI language | German, one catalog `ui/src/lib/i18n/de.ts`, no switcher |
| Frontend | Svelte 5 + Vite + TypeScript, no SvelteKit, no animation library, Lucide icons only |
| Keys | only inside fields/dialogs: Tab/Shift+Tab, Enter = save, Esc = cancel, Ctrl/Cmd+C/V/X/A/Z. Amended by the input audit (2026-09-24): fields take every character of the layout (AltGr on Windows, Option on macOS) and the OS editing keys (word/line moves, delete word, redo, Shift selection); Tab/Shift+Tab move the focus everywhere and Enter/Space press the focused control (no dead end after a field); a modal dialog holds the focus; Cmd+, reaches the macOS menu. Still no own shortcuts and no WebView shortcut |
| OS window functions | keep Alt+F4, Cmd+Q/W/M/H, double-click on title bar; no own shortcuts |
| Mac | no Mac available: macOS via GitHub `macos-latest` (real app screenshots, dmg install probe, keychain test) + WebKit locally |
| macOS minimum | 14.0 (Safari 17 baseline, `data_store_identifier` for sessions) |
| Evaluation data | no access to Katharina: local real data + real runs through the app, two realistic invented profiles, blind labels by two independent agents + tie-breaker |
| Embeddings | dropped (user, 2026-09-24): the rule engine covers the measured failures; the `Embedder` seam stays for later |
| AI stage | two-stage like professional systems: stage 1 = our engine for every job (incl. the skill rubric); stage 2 = the improved `job-matching` skill, optional, only for the app's top matches, in the user's own Claude (no API key). The app writes a machine-readable top-matches file for it |
| Reuse | the app is generic: everything personal lives in the profile; competences may carry alternative terms (`auch`); lexicon = general core + domain packs that activate automatically from the profile; no pack editor in the UI; new portals via adapters |
| Extra criteria | superseded: the engine adopts the skill rubric (contract type, permanent-role salary and region, seniority, formal requirements) via optional profile keys - exclusions only on clear wording, otherwise checks |
| Scraping | everything switchable per portal (Active / Fetch details / Sign in), safe defaults, risk badge per switch |
| HTML overview | no full text: title, company, location, portal, link, match (excluded: ring without number), up to 2 met requirements (the list keeps no open ones), exclusion reason in words |
| Extras | Pin (star) + auto fetch on start (> 6 h, switchable); no notifications, no "still open?" checks |
| Logo | no CXpertise company logo; the coral app icon (folder + check) is the window icon of the native title bar and the mark of the first run and empty states |
| Heading colour | warm dark ink (45 7% 17%), not slate; coral is the only accent colour (user chose variant A). Headings stay ink; "only accent colour" is superseded by "cxpertise navy" below |
| Windows caption buttons | superseded (2026-09-24 night): the native caption buttons of the Windows title bar |
| Sizes | controls 28/36/40, list rows 86 (fixed, one-line title, date top right), body text 15 (the top strip is gone: the native title bar of the OS) |
| Layout | variant C chosen by the user: calm sidebar (~196 px, no own surface, hairline divider, nav with icons and unread count, quiet run status at the bottom; icons only below ~1100 px); search, "Abrufen" and filters in the list column header (revised 2026-09-24 night: no content strip, the native title bar) |
| Toasts | allowed for short confirmations whose result is not visible otherwise (saved, copied, files written, run finished): bottom right, at most 3, ~4 s, paused on hover; anything needing action stays inline |
| User test of the installed app (2026-09-24 evening) | Windows title bar like a native one (full width, 16 px app icon + app name at the left, caption buttons at the native height, no tooltips); macOS uses the normal native title bar; "Abrufen" lives in the list column header next to the search; the cxpertise palette again: light coral (13 73% 63%) for primary fills, hover 13 64% 56%, switches coral when on; lighter font weights; faster, snappier motion; no lag in the real app; native-feeling input (left click only for controls, middle-button scrolling in scroll areas, copyable text where it makes sense); no unneeded micro details |
| UI round 2 (design critique) | one white sheet for all views (no floating cards), coral only for Abrufen, selection bar, unread dot, active nav (progress bars stay coral as Abrufen feedback) - amended by "cxpertise navy": the selection bar, active nav and progress are navy now; mid scores ochre; primary in deep coral (4.9:1); reader like an issue view (title, facts, match line, chips, actions); sort as icon toggle; switches ink when on |
| Cleanup outside | `.notes` archived to `../_archive/TEST-notes`; user deletes `origin/ci-macos` and release `latest`; CI publishes nothing |
| Native window frame on both OS (user, 2026-09-24 night) | Windows keeps its native title bar (icon, title, caption buttons, system menu on the icon, right click and Alt+Space, snap layouts), coloured like the app via DWM: caption = `--bg` cream (= `backgroundColor`), title = ink, dimmed to `--text-subtle` while inactive (Windows 11; Windows 10 keeps its light bar); macOS keeps its native title bar with the centred title (revised: the unified title bar, see "Title bars, final"). No title bar or caption buttons in the page; the sidebar has no brand row. Principle: two versions, as identical as possible inside the window; whatever differs by OS convention does differ (see Platforms) |
| UI overhaul after the installed test (2026-09-24 night) | Abrufen next to the search in the list header (Abbrechen in its place during a run); light coral primary, coral switches; weights 400/500/600; motion 100/150/180 ms, ring fill 360 ms, ease-out, no stagger, no bounce, no glow/lift/shimmer, no backdrop blur (amended by "cxpertise navy": 80 ms hover-in, press scale, icon nudges, sliding indicators, the one-shot pop, draw and flash are allowed); rows and rings do not replay when a view comes back; one Tauri channel per streaming call (a shared one lost the second run); day overview without a profile: tiles Neu and Ohne Details plus a card to choose one; each portal problem once; a Gemerkt tile once something is pinned; excluded unread jobs under Neu behind the divider |
| Title bars, final (user, 2026-09-24 late night) | Windows: the native title bar stays (the 36 px web-drawn bar and a snap-layouts overlay were tried and dropped); its colours are named constants in `platform.rs` (`TITLE_BAR_BACKGROUND`, `TITLE_BAR_TEXT`, `TITLE_BAR_TEXT_INACTIVE`, each checked against its token), so a later cxpertise blue is a one-line change there and in `tokens.css`. macOS: the unified title bar of Mail or Notes: `titleBarStyle` Overlay, hidden title, traffic lights at x 20 / y 27 (the CI smoke measured the 14 px buttons at centre 16 with y 18, the toolbar controls sit at centre 25, so y 27 centres them; checked by the macOS CI screenshot and the smoke line `SMOKE {"lights":...}`), placed by the app itself (`platform.rs` `lights`, tao's geometry after show and on every resize, focus, theme and scale change) because tao applies the inset only while its covered content view draws; the sidebar runs to the top with the lights in its first row, search and Abrufen sit in that row, the sheet reaches the top edge, and every empty point of the row moves the window (`data-tauri-drag-region` on `DragBand` and the full-width list header row, macOS only, own capability `macos.json`; the harness probes the whole row in every view and width); a double click there zooms (native zoom through Tauri's drag script; the system setting "double-click a title bar to" is not read). Minimum window 480 x 360 so every Windows 11 snap layout fits (quarters of 1366 x 768 included); snap layouts are the native ones. Later option: WebView2 `CoreWebView2WindowControlsOverlay` once it is stable |
| Warm selection (user, after the A/B preview, 2026-09-24) | supersedes the navy selection below: the selected row and the active nav are warm and very light (row wash 22 72% 96.5 %, hover 94.5 %, a coral bar at 0.75, the ring track 20 40% 88 %; the nav pill white with an ink label and a coral icon; soft count pills 95 % with coral-800 digits). Navy stays for the sidebar count pill, the pinned star, tooltips, progress, info, links, focus, the first-run current step and the chosen filter. Switch rows get no background at all ("kein grau, nur die Schalter"): only the switch reacts, also while the pointer is on its row's text, which still toggles it. No press ever deforms a control: buttons give uniformly (0.98), everything else only darkens; rows share one grid (a row's wash and divider are its own box, the container's inset is --row-inset) |
| cxpertise navy for structure and state (user, 2026-09-24) | two brand colours with two jobs: coral (13 73% 63%) means act or new - the one primary per view, switches that are on, the unread dot; navy (212 34% 37%) and deep navy (212 30% 26%) mean where you are and what the data says - the selection bar and wash, the active nav (a sliding white pill with a navy label), the chosen filter, focus, caret and text selection, progress, counts (a deep navy pill in the sidebar, soft pills elsewhere), tooltips, info, sub-labels and links. Navy only as small dense marks and 93-96 % washes, never a large fill, never on headings, switches, scores or row-title hover; navy and coral never share an element and never blend. Motion "quiet at rest, rich on contact": hover-in 80 ms, hover-out 150 ms, press 60 ms with a small scale (0.97 / 0.94 icon / 0.985 tiles), release with --ease-emphasized; icons nudge 1-2 px, indicators slide 180 ms, counts roll, the star pops once (1, 1.18, 1), checks draw, a passage flashes; no lift, glow, stagger, bounce, blur or replay (nothing animates on mount; `intro: false`) |
| Profile editor (user, 2026-09-24) | The Profil view is the profile as a form (no JSON writing): the keys the engine and the skill read, grouped as a consultant thinks (Person, Kompetenzen und Schwerpunkte, Erfahrung, Werkzeuge und Zertifikate, Sprachen, Wünsche, Ausschlusskriterien); chip fields for lists, toggle buttons for small fixed choices (no dropdown). Three ways in without a profile: Profil anlegen, Aus Lebenslauf erstellen (a German prompt for the user's own Claude, `core/src/profile/prompt.rs`, the answer is pasted back), Datei wählen; file and answer fill the form for review. Saving merges into the JSON: only changed fields are written, unknown keys, their values and the key order stay, atomic write, one backup `profil/beraterprofil.json.bak`. "Vorlage speichern" is gone. New profile inputs `schwerpunkte` (at most 5, stars on the competences), `wunschrollen`, wishes in `einsatzpraeferenzen` (`tagessatz_wunsch`, `remote` voll/ueberwiegend/teilweise/vor_ort, `regionen`, `branchen`); the engine side follows separately |
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
(list and counts from ONE query) · `job_detail(key)` · `mark_read(key) -> bool` · `set_pinned(key, on)` · `pick_profile -> ProfileDraft?` ·
`parse_profile(text) -> ProfileDraft` · `profile_prompt` · `save_profile(ProfileSave{before, after, source?}) -> ProfileInfo` ·
`remove_profile` · `save_mailbox` · `remove_mailbox` · `portal_login` · `portal_logout` ·
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
`Highlight{id, start, end (UTF-16), kind, reason}` · `ProfileInfo{fileName, bytes, savedAt, quality: good|thin|empty, understood{competenceCount, competences[], sources[], criteria[], warnings[], packs[], years, degrees[], focus[], roles[], wishes}, scoredAt, pending, form}` ·
`ProfileForm` (the editor's fields, `core/src/profile/form.rs`) · `ProfileDraft{form, source, quality}` ·
`PortalHealth = ok | paused{until, reason} | quotaReached{until} | layoutSuspect{emptyMails, pages} | loginRequired` ·
`AppState{platform, dryRun, firstRun, running, settings, mailbox, profile, portals[{portal, enabled, fetchDetails, login, loginEnabled, signedIn, risk: low|grey|account, health, quota?}], autoFetchOnStart, lastRun, counts, topMatches[<=5], matchPending, dataDir, logDir, resetReport?}` ·
`CommandError{kind, params}`. Traits: `pipeline::score::Matcher{rev, assess}` · `portal::PortalAdapter` · `matching::prescore`.

### Matching engine (`core/src/matching`, pure, synchronous, integer only)
Modules: mod, params, types, lexicon/ (external contract), normalize, profile, sections, requirements, signals,
criteria, ladder, relevance, semantic (feature), score, explain.
- Profile compiled once: core phrases with JSON path, languages + level, degrees, total years, criteria, quality
  (empty => not scored). Profile keys stay German (the existing profile format); English aliases also understood.
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
- Private gold set (ignored, never committed): real local ads + real ads fetched by normal app runs from the owner's
  alert mails; three realistic composite profiles modelled on real consultant CVs (interim CFO/controlling, SAP FI/CO
  consultant, IT project lead; no real person, user decision 2026-09-24); the primary one is modelled on the skill
  rubric, which belongs to Katharina (very senior, ~30 years, Diplom-Kauffrau, interim >= 1000 EUR/day, permanent >= 150k,
  DACH, Munich region for permanent roles); blind labels 0-3 by two Opus agents with the
  skill rubric, a third breaks ties; old engine vs new engine vs labels.
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
- Shell (revised 2026-09-24 late night, see Decisions "Title bars, final"): Windows shows its native title bar, below
  it the sidebar (nav Jobs · Profil · Einstellungen, the run status) and the white sheet; macOS shows the same under a
  unified title bar (52 px toolbar row with the traffic lights over the sidebar). "Abrufen" next to the search in the
  list column header. No menu (Windows), no gear icon. Closing during a run shows a short note until the run stops.
  Every view switch is the same 100 ms cross-fade (new view on top, never an empty sheet); nothing animates at start;
  `:root[data-window]` is 'inactive' while the OS window is in the background (selections grey out against it).
  The sidebar run status shows only while there is a run to open.
- Jobs: toolbar (Abrufen primary lg / Abbrechen · Neu n | Alle n · Beste Passung | Neueste · search) · left column
  run card + list (72 px rows: ring 40, title with unread dot, meta, reason line, date, status badge only on deviation;
  excluded grey behind divider; duplicates as one row) · reader card 720 px (ring 96 counting up, band word, n of m must,
  hard-criteria strip, reasons met/open/check/violations, hover = tooltip + highlight, click = scroll to passage) ·
  day overview when nothing is selected (3 stat tiles, unread per portal, best 3, pinned, open issues, "Übersicht öffnen").
- Profil: the profile as a form (see Decisions "Profile editor"): head card (file, quality, one line of what the app
  understood, what it could not use, rescore, Datei wählen / Entfernen), sections, sticky save bar (Speichern only with
  a change, Verwerfen), a question before leaving with unsaved changes; empty state with the three ways in.
- Einstellungen: Postfach · Abruf (auto fetch) · Portale (switches with risk badges, health, quota only >= 80 %) ·
  Dateien · Wartung. First run: full page with three real, self-ticking steps.
- All states per screen (first use, no profile, empty, loading, run, nothing new, no search hit, errors, offline,
  portal paused, no details, teaser, unscorable, excluded). Feedback where the action happened; no toasts.
- Tokens (`tokens.css`, `:root`, light only, `color-scheme: light`): palette from the brief (coral 13 73% 63%, navy
  212 34% 37% with deep navy 212 30% 26% and washes 96/93/90/84 %, cream 32 33% 96%, ink 45 7% 17%, ...) plus shades
  (coral-800 13 62% 45% for text, *-strong/*-soft for status; info is navy), semantic tokens only in components, score
  colours (high 152 50% 31%, mid ochre 38 55% 55%, low 30 5% 56% with text 30 4% 42%), no decorative gradients or glow
  (one gradient: the light of a loading placeholder); brand mark = the real coral app icon (folder + check) as SVG;
  small shadows only for what floats (dialog, toast, tooltip) plus the static hover shadow on `::after`, none
  animated; radii 6/8/10/12/16, 4 px spacing, controls 28/36/40, type 12/13/14 (tabs)/15/15/17/20/26/34 (UI standard
  15/22), weights 400/500/600, motion 60/80 (hover-in)/100/150/180 ms, ring fill 360 ms, loop 1400 ms, ease-out and
  `--ease-emphasized`, no stagger, no bounce; window tokens `--mac-toolbar` 52 px and `--traffic-lights-width` 80 px
  (macOS row, checked against `tauri.macos.conf.json`).
- 29 components (Button primary|secondary|ghost|danger|link x sm|md|lg, Count, Icon, IconTile, BrandMark, Card, Badge, Segmented,
  Toggle, TextField+Field, SideNav, ScoreRing, Meter, Skeleton, Spinner, Notice, EmptyState, StatTile, StatusLine,
  Dialog, Tooltip, Disclosure, SettingRow, ListRow/JobRow, ReasonItem, Toast, DragBand (macOS toolbar row)). Not:
  select, checkbox, radio, context menu, window controls.
- Motion: only transform/opacity (colour on hover); shadows/glow on `::after` via opacity; whole-pixel end values;
  <= 10 staggered, <= 10 rings animating, FLIP <= 100 rows else cross-fade; reduced motion via `motion.ts`.
  Since "cxpertise navy": hover-in `--dur-hover` 80 ms on the :hover rule, hover-out 150 ms on the base rule, press
  60 ms with `--scale-press*`, `--ease-emphasized` for everything that slides or settles; transforms on HTML
  wrappers, never on SVG children; one-shots only from event handlers or a mounted previous-value compare; every
  scale, move and turn token has a neutral reduced-motion value (a half turn keeps its angle).
- Consistency: stylelint (no hex/named colours, no colour functions/units outside tokens, strict values, allowed
  transition properties, keyframes only in motion.css) · ESLint (no inline styles, raw elements only in components,
  restricted imports, no title attribute, no empty catch, listeners only in input.ts) · Rust architecture tests ·
  gallery · Playwright screenshots in Chromium + WebKit against `vite preview` with production CSP · per-screen audit.
- Input policy: prevent contextmenu everywhere, non-left buttons, dblclick outside drag area, dragstart, selection
  outside fields, keys outside fields (except Tab and Enter/Space on controls, see Decisions "Keys"), Ctrl/Cmd+wheel
  (a wheel listener only while Ctrl/Cmd is held), pinch. Native: WebView2 switches, macOS minimal menu,
  `accept_first_mouse`, no link preview, devtools off in release, navigation guard, window shown after first load.

### Platforms (documented differences only)
Inside the window both OS show the same app; these differ by OS convention (UI: `ui/src/lib/platform.ts`, native:
`src-tauri/src/platform.rs`): native window frame (Windows title bar in the app's colours via DWM, dimmed title while
inactive; macOS unified title bar: traffic lights over the sidebar in a 52 px toolbar row that holds search and
Abrufen and moves the window, no title text) · dialog buttons (Windows: action first; macOS: cancel left, action
right) · scrollbars (Windows: slim styled, shown over their scroller; macOS: native overlay scrollbars) · middle-button
autoscroll (Windows; macOS has none) · words for OS things (Explorer / Finder, Anmeldeinformationsverwaltung /
Schlüsselbund) · menu (none vs. minimal App/Edit/Window) · font smoothing on macOS · keychain vs. credential manager
(same code) · session storage API · reveal in folder (`explorer /select` vs. `open -R`) · per-OS user agent. Build target Safari 17; forbidden: View Transitions, `@starting-style`,
`scrollbar-gutter`, `content-visibility`. Windows: NSIS currentUser, German installer, downloadBootstrapper.
macOS: Apple Silicon only (M1 and newer, since 2020; user 2026-09-24), ad-hoc signed, minimum 14.0; the icon targets the macOS 26 (Tahoe) Dock look.

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
- [x] 1b UI foundation (merged 587dbec): package.json, Vite, Svelte, TS; tokens/base/motion css, motion.ts, input.ts, api.ts, i18n;
      shell (title bar, tabs, caption buttons); gallery with token boards; Playwright against `vite preview` with
      production CSP; lint setup; new `ui_contract.rs`; old `ui/*` removed.
      Done when: `npm run check` green; a planted hex value, px value and raw `<button>` each turn lint red.
- [x] 1c Platform and delivery (merged 92669ef; macOS parts verified only by CI): tauri.conf (withGlobalTauri false, freezePrototype true, frontendDist `../ui/dist`,
      beforeBuildCommand, hidden window + light theme), `tauri.windows.conf.json` (NSIS), macOS minimum 14.0,
      `platform.rs`, smoke on `data-testid`, `rust-toolchain.toml`, English CI with `tauri build` on both OS,
      macOS dmg install probe with screenshot, no public release.
      Done when: release app starts on Windows and in macOS CI without CSP violation; CI ships setup.exe + dmg.
- [x] 1d Evaluation base + port (merged 9c8279f; 35/35 old tests, 80/80 legacy.json, 27/27 edge cases, 12/12 local real ads): corpus to K40 + second profile, `corpus.json`, `legacy_baseline.py` -> `legacy.json`
      FIRST; `eval.rs`; port "v3-equal" with `legacy_percent`; `matching_legacy.rs` (35 tests).
      Done when: fidelity 31/18/11/7/6/0 reproduced, 35 old tests green (1 documented deviation),
      `legacy_percent == legacy.json` for all K, clippy clean.

### Phase 2 - core work (parallel, disjoint files; contracts frozen)
- [x] 2A Matching better (merged 9c8279f; in band fin 17->38/40, it 30->39/40, band distance 491->3 and 188->1, ~0.25 ms/job): V5, V6, V3, V2, V1, V4, V15, V7, V9, V8, V16, V10-V13, decided/check model, V17, V18, V19,
      explain.rs, prescore - one commit each with corpus guard; calibrate, freeze.
- [ ] 2B Store, runs, export (done with 1a and the integration track except: alsoOn, freelance.de guest teaser; the engine is wired as `LocalMatcher`, see `docs/MATCHING.md`): schema 3 + chain + WAL; save_match(es), mark_read, set_pinned, job_page; settings
      (portal switches, autoFetchOnStart); LocalMatcher, scoring at JobUpdated + catch-up, Rust-triggered rescore and
      auto fetch; profile summary + template; Excel column + grey header, mail address out of info sheet; HTML
      overview; TXT byte tests; demo with high/mid/low/excluded.
- [x] 2C Components (merged 587dbec; 78 harness tests, both engines): all 23 with variants, states, motion; complete gallery; baselines.
- [x] 2D Scraping and sign-in (session delete, macOS data store, no unasked sign-in window, keychain test already merged with 1c): S1-S11; switches honoured in the fetch path; optional sign-in with risk note; delete
      session per portal (macOS `data_store_identifier`); keychain test on macOS; dead code list.
      Left for the integrator: `AppBackends::prescore` -> `matching::prescore` with the profile (neutral until then);
      `commands/mod.rs` could use `sync::lock`; exports (Excel, TXT) still list duplicate rows (the list shows one).
      Done when: 26 fetch tests + new (4th test portal via registry only, health, teaser, Retry-After, requeue, slug
      URL = same id, duplicate group, IMAP loads candidates only, details off => zero portal requests, sign-out
      verified on both OS).
- [x] Integration: engine wired (LocalMatcher, rescore, job detail, profile summary, template, top_matches.json), scraping merged, prescore orders the fetch queue; engine v3 with the skill rubric, domain packs and aliases (in band 49/51/49/51 of 52 for the four profiles). A + B done (LocalMatcher, Rust-triggered
      rescore, reader recompute, profile summary, template, demo on the real engine, `auswertung/top_matches.json` for the
      skill as optional stage 2); D open (prescore hook not exposed by the fetch queue yet).

### Phase 3 - screens and core workflow (two UI agents)
- [x] Jobs (toolbar, run card, list with progressive rendering, reader with reasons and highlights, day overview)
- [x] Shell, Profil, Einstellungen, first run; all states; texts only from `de.ts`; `mark_read` only on a real click
- [x] Profile editor (form over the profile JSON, merge with one backup, Claude answer, chip fields, Schwerpunkte,
      wishes; 17 harness scenarios in both engines, baselines `profile`, `profile-empty`, `profile-paste`)
- [x] >= 30 harness scenarios (200 in Chromium + WebKit after the polish round) in Chromium + WebKit; screenshot baselines; smoke probe of the real app
- Done when: core workflow works in both engines and the real app; every view in every state is captured; 0 lint
  exceptions; all 33 audit findings of the old UI are resolved. Send screenshots (Windows + macOS CI) to the user.

### Phase 4 - English sweep (parallel to phase 3)
- [x] Translate remaining comments, logs, errors, asserts, CI, hook, toml; new English README; `language.rs` with allowlist
- [x] Finish the dead-code list

### Phase 5 - verification, measurement, audit
- [ ] Real runs through the app (gold set within limits and switches), blind labels, old/new report in `docs/MATCHING.md`
      Run 9 on the test mailbox, fixed: portal promo/onboarding mails are no alerts (no false "layout changed?"),
      a collection mail never takes another job's title as company or location, a stored title-like pair gives
      way, a new location makes the score pending (fixtures `promo_mails/`, `forward_composite.eml`).
- [ ] Live canary per portal (one counted page via `admit`)
- [ ] Windows installer + first run + screenshots; macOS CI screenshots + dmg probe + WebKit scenarios
- [ ] Performance (start time, long tasks at 2000 jobs), contrast
- [ ] Consistency audit per screen (checklist below) and fixes; one adversarial review workflow over the whole diff

### Phase 6 - delivery
- [x] Skill `job-matching` (stage 2): back up the original, drop the hard-coded foreign path (use the app's work folder, works on macOS), read the app's top-matches file instead of screening every ad, align the rubric wording with the engine, test, deliver as a folder with a short install guide (`tools/job-matching-skill/`: `SKILL.md`, `scripts/matching.py` brief + render with the rubric caps, README, test on the corpus; original backed up outside the repo)
- [ ] Final CI builds (artifacts only), first-start guide (SmartScreen, Gatekeeper, keychain), close this plan, hand over

## Consistency audit per screen
For each of Jobs, Reader, Day overview, Profil, Einstellungen, First run, dialogs:
- [ ] tokens only · 4 px grid, shared edges aligned · <= 1 primary, button variants by rule · icon sizes by context
- [ ] hover/active/focus/disabled everywhere · all screen states present · glossary, no sentence twice · tabular numbers
- [ ] motion only via tokens, reduced motion checked · AA contrast (documented exception: primary button)
- [ ] Windows and macOS screenshots side by side: only the documented differences

## Glossary (UI)
Job · Portal · Passung · Details · Abrufen · Profil · Postfach · Alert-Mail · Übersicht · Ausgeschlossen · Neu (= unread) ·
Zu prüfen · Merken. Checked for the UI catalog (`ui_contract.rs`) and the Rust texts: exports, startup dialog, window titles, file dialogs, macOS menu (`rust_texts.rs`).


## Budget and models
The user's usage limit is tight: work token-efficiently without lowering quality - targeted reads, focused test runs,
no sub-agents inside tracks, screenshots only at milestones, commit every finished step. Models (no Haiku):
- **Opus** for everything that shapes the product or the measurement: matching engine, contract, store/pipeline,
  scraping/sign-in logic, UI foundation, components and screens, integration and merges, corpus band review,
  gold-set labelling, consistency audit and the final review.
- **Sonnet** for simple, fully verifiable work: phase 4 comment/log translation (checked by language.rs, build and
  tests), README and first-start guide from finished facts, collecting CI artifacts and screenshots, routine cleanup.

## Status 2026-09-24 evening (pause until the usage limit resets)
Done on main (pushed, CI green on Windows and a real macOS runner before the last push): phases 0-4, engine v3,
scraping, all screens with navigation variant C and the polish round, macOS-shaped app icon, improved optional skill.
Next: (1) check the CI run of e67e2ac incl. the macOS Dock screenshot of the new icon (shadow ok on macOS 26?);
(2) independent design critique of every screen (screenshots in both engines) and fixes; (3) `ProfileUnderstanding`
gets `packs`, `years`, `degrees` in view.rs (UI already renders them); (4) phase 5 real-data measurement - needs the
user to connect the mailbox in the app first; (5) README + first-start guide, final review, installer on this PC.
