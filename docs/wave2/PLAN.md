# Cloud plan: finish Job-Alert-Monitor (wave 2)

You are Claude Code in a cloud session on a Linux clone of github.com/CXecutives/testt. This file is your complete brief:
context, rules, every task in order with its definition of done, and how to work economically. When every box below is
ticked (or explicitly skipped with a reason), the app is finished. Do only what is listed here: no new features, no new
audits, no speculative refactors.

## 1. The app in one paragraph
Job-Alert-Monitor is a desktop app (Tauri 2 + Rust, Windows and macOS) for a freelance or interim consultant. It reads
job alert mails (LinkedIn, freelancermap, freelance.de) from Gmail over IMAP, fetches the job pages conservatively,
scores every job against the consultant's profile with a local integer matching engine, and shows jobs like a mail app
(Eingang, Archiv, Papierkorb, a favourite star) with a reader that explains the score. It writes Excel, TXT and an HTML
overview. UI: Svelte 5 + Vite + TypeScript, German source catalog with an English mirror. Layout:
- `core/` (`jobalert-core`, no UI prose): `mail/`, `portal/`, `fetch/`, `matching/`, `store/` (SQLite), `pipeline/`,
  `export/`, `profile/`, `view.rs` (IPC DTOs generated to TypeScript with ts-rs).
- `src-tauri/`: app start, `platform.rs` (the only per-OS code), `commands/` (IPC).
- `ui/src/`: `styles/tokens.css`, `components/` (design system), `features/` (screens), `lib/` (input, platform, ipc,
  i18n, motion, state).
- `tools/ui-harness/`: Playwright specs against the real UI with a stub backend (`stub.ts`), Chromium and WebKit.
`CLAUDE.md` is binding and short: read it fully first. `docs/PLAN.md` holds every product decision (long: search it
with grep for the topic at hand, do not read it whole). `docs/MATCHING.md` documents the engine.

## 2. Rules you must never break
- Never touch the old repo CXecutives/projektscraper or any `legacy-*` branch. Work only on branch `wave2`; never push
  to `main`, never force-push, never delete remote branches. Open a pull request at the end; do not merge it.
- No secrets: never ask for, create, log or commit passwords, tokens or keys. No personal data in the repo (it is
  public); `core/tests/fixtures/private/` does not exist in your clone and must not be recreated.
- No live network requests to LinkedIn, freelancermap, freelance.de or Gmail, not even for testing.
- Everything in the repo is English (code, comments, docs, tests, commit messages), except `ui/src/lib/i18n/de.ts`
  (the German source catalog; `en.ts` mirrors it with the same type) and files marked
  "external contract - do not translate". `core/tests/language.rs` enforces it.
- The backend never sends prose to the UI: notices and errors are `{code, params}`; the texts live in the catalogs.
- UI architecture (lint and `core/tests/ui_contract.rs` enforce most): styling only via tokens in
  `ui/src/styles/tokens.css`; controls only from `ui/src/components/`; all input handling only in
  `ui/src/lib/input/input.ts`; per-OS decisions only in `ui/src/lib/platform.ts` and `src-tauri/src/platform.rs`;
  motion only via `ui/src/lib/motion/` (quick, <= 180 ms, ease-out, no bounce); Tauri only via
  `ui/src/lib/ipc/api.ts`; at most one primary button per view; light mode only.
- UI text rules: little text, only what is needed, plain and human. No colons in labels or headings, no "X: Y", no dashes
  or em dashes as separators, no exclamation marks, no emoji, no filler. Buttons are one verb phrase, a note is one short
  sentence. One word per thing (the glossary in docs/PLAN.md). German uses "du". Typographic quotes „…“ in German.
- The user's decisions that stand (do not undo): native title bars on Windows and macOS; no manual sidebar fold; no count
  in the sidebar (the list's filter segments "Neu n / Alle n / Favoriten n" keep theirs); only the switch itself toggles
  (its row text is no click target); only the left mouse button presses anything; a press beside a text field ends its
  focus; one keyboard-only focus ring; score rings without dashes; the coral selection bar slides between rows; no
  application stages, no follow-up, no notes; no risk grades on portals.
- TXT export files stay byte-identical (`header_is_exactly_the_contract`, `txt_is_blind_to_the_match`).
- Matching: a change of scores needs `ENGINE_VERSION` + 1 in `core/src/matching/mod.rs` and a new `GOLDEN_DIGEST` in
  `core/tests/matching_corpus.rs` (the assertion prints the new value as "left"); every held-out floor in
  `core/tests/matching_heldout.rs` must still hold; add a short section to docs/MATCHING.md.

## 3. Setup (Linux)
```
npm ci
npx playwright install --with-deps chromium webkit
sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev   # needed to compile src-tauri; skip if not allowed
git checkout -b wave2 origin/wave2 2>/dev/null || git checkout -b wave2
```
If the system packages cannot be installed, use `cargo test -p jobalert-core` and `cargo clippy -p jobalert-core
--all-targets -- -D warnings` and say in the pull request that `src-tauri` was checked by CI only.

## 4. How to work economically (full quality, few tokens)
- Read `CLAUDE.md` once. For everything else read only what the current item needs: the cited files and lines, a grep
  for the symbol, the relevant spec file. Do not explore the repo broadly.
- Before fixing, verify: many items were fixed after they were found. A 30-second check of the cited code decides.
  Skip fixed items (note them), never re-fix.
- Group work by file: when you open a file for one item, do every item in this plan that touches that file.
- Tests: add or adjust a focused test for every behavioural change. While working run only the touched spec files
  (`npx playwright test -c tools/ui-harness/playwright.config.ts --project=chromium <spec>`) and the touched Rust tests.
  Run the full gates (section 7) only before each push, and push after every 3-6 finished items.
- Keep changes minimal and in the style of the surrounding code (its comment density, naming, idiom).
- If an item is ambiguous, pick the option that follows the documented decisions and the user's priorities (logic,
  consistency, little text, clean and modern), and note the choice in the commit message. Do not ask questions.
- Keep `docs/wave2/PROGRESS.md` (create it) up to date: one line per item, done / skipped (why) / open. Commit it with
  each push. If the budget runs low: push, update PROGRESS.md, open the pull request.

## 5. Tasks, in this order
Inputs in this folder: `features.json` (the chosen features with full specs), `findings.json` (102 findings of an
independent audit on commit a2b2353: id, lens, title, severity, files, evidence, repro, fix, verification, verifier
notes; `betterFix` in the notes wins over `fix`), `docs-edits.json` (51 drafted doc edits). A repro path starting with
`<local scratch>` pointed to the auditor's local probe: rebuild the repro from its description.

### 5.1 Features (the user chose them)
- [ ] **features-01, the reader's terms strip**: the reader ("Rahmen" strip, `ui/src/features/jobs/Reader.svelte`)
      always shows the ad's day rate and start as plain chips from the job's facts, like the list row does, whether or
      not the profile sets a minimum rate. No new section, no new label; in the "all met" one-line form the values join
      that line. Done when: a job with a rate and a start shows both in the reader for a profile without a minimum
      rate (stub scenario or a new one), in German and English, with a harness test.
- [ ] **features-02, search**: the list search matches every word of the query in any field (title, company, location,
      ad text) plus the portal name ("SAP Hamburg", "linkedin"), case- and accent-insensitive like today; nothing
      changes on screen. The search lives in the store (`core/src/store/jobs.rs`, `search_text`, `like_pattern`) and
      the stub (`tools/ui-harness/stub.ts`) must behave the same. Done when: Rust tests for multi-word, portal name,
      umlauts, empty query; a harness test; the counts ("Auch im Archiv (n)") follow the same query.
- [ ] **features-07, dead code**: remove the unreachable tile and portal filter code (`jobs.setFilter`, `JobFilter`,
      `matches()`, the filter state in `ui/src/lib/state/jobs.svelte.ts`, and whatever only they used). Done when:
      `npm run check` is clean and nothing else changes.

### 5.2 Findings (`findings.json`), by severity
- [ ] **The 5 high ones first**: ui-forms-01 (a refused value in the hidden Festanstellung block makes Speichern
      fail), export-matching-01 (own_text matches heading prefixes, so an ordinary requirement line can cut the ad; a
      matching change, see the matching rule in section 2), ui-jobs-01 (arrow keys, Home and End follow the backend row
      order, not the order on screen), live-forms-01 (from a CV, Esc in the answer field or leaving the view throws the
      pasted answer away), live-forms-02 (text typed into a chip field is no change, so Speichern stays disabled).
- [ ] **The 49 medium ones**, grouped by file. The names lens (21) makes labels and terms consistent across the UI, the
      exports and the macOS menu: apply its fixes to both catalogs and to Rust texts (`core/src/export/texts.rs`,
      `src-tauri/src/platform.rs`) together, so one concept has one word everywhere.
- [ ] **The 48 low ones**, as long as the budget lasts; skip pure taste.

### 5.3 Review of the merges that had no second look
The last round merged four tracks without their independent review (the agents were stopped by a usage limit):
- [ ] The shell track (`git log --merges --grep "shell track"`; its WIP commit `4a65b1a`): input, platform, shell and
      shared components, the Windows scrollbar width (list edges and reader tools stay put between long and short
      jobs), Shift+Arrow multi-select, arrow keys scrolling Einstellungen and Profil, dialog confirm verbs, notices,
      toasts. Read its diff; fix whatever is wrong, half done or against the rules, with tests
      (`tools/ui-harness/specs/wave1-shell.spec.ts`).
- [ ] A quick read of the backend track merge, the rings+bar merge and the CV prompt merge for obvious defects.

### 5.4 Text amount
- [ ] Read every string of `ui/src/lib/i18n/de.ts` where it appears (serve the harness build and look at each screen;
      `?lang=en` for English; the stub scenarios in `tools/ui-harness/stub.ts`: first-run, no-profile, empty, many,
      running, paused, offline, list-error, profile-thin, profile-broken, reset, ...). Cut what is too long, redundant
      (repeats a heading or what is visible), explains the obvious, or could go. Mirror every change in `en.ts`. Keep
      tests green (texts asserted in specs change with them).

### 5.5 Documentation and the plan
- [ ] Apply the doc edits of `docs-edits.json` that are still true (skip MATCHING.md versions 12 to 14 and the plan boxes
      "Live canary" and "Performance", done already).
- [ ] `docs/PLAN.md` phase boxes: tick what the code proves done ("2B Store, runs, export": check alsoOn and the
      freelance.de guest teaser in the code), and reword "Real runs through the app" and "Final CI builds, first-start
      guide, close this plan, hand over" to say what is done (README has the first-start guide; the evaluation used
      eight blind held-out sets instead of a private gold set). Add one line for wave 2.
- [ ] README.md: true for the current app (no stale mention of a sidebar fold, Ctrl/Cmd+B, a page-drawn title bar,
      risk grades or the sidebar count).

### 5.6 Finish
- [ ] Full gates green (section 7).
- [ ] Delete `docs/wave2/` in the last commit (keep PROGRESS.md content in the pull request description instead).
- [ ] Push `wave2`, open the pull request against `main` titled "Wave 2: last findings, search, reader terms, texts".
      Its description: what was done, skipped (with reasons) and left open; which screenshot baselines changed by
      intent (the integrator refreshes them on Windows); anything the user must decide.

## 6. Not in scope (do not do)
New features beyond 5.1, new audits or held-out sets, engine tuning beyond a finding's fix, updating or committing
baseline PNGs, building installers, touching CI workflows, the open decision on hourly wages of student and agency jobs
(read as a day rate x 8 today; the user decides), code signing.

## 7. Gates (all must pass before each push)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
npx playwright test -c tools/ui-harness/playwright.config.ts --project=chromium --project=webkit --grep-invert "baseline:"
python3 tools/job-matching-skill/tests/test_matching.py
```
The `baseline: ...` screenshot tests are Windows images (CI runs the harness on windows-latest); on Linux they differ,
so they are excluded above. A test that fails only under load: re-run it alone before blaming a change.

## 8. Pitfalls seen in this repo
- `de.ts` and `en.ts` must have the same keys (a type error otherwise).
- `core/tests/language.rs` flags German words in comments, also in string lines that start with `*` in .rs files.
- `core/tests/ui_contract.rs` checks pressed styles (`:where(:root:not([data-aux-press]))` guard with `:active:hover`),
  focus rings, text rules, token use.
- A harness test that reads the reader must scope to the open stage (`getByTestId('stage')`) and wait with
  `expect.poll` for the settled state: the leaving stage and late layout cause flaky results.
- Run events must stay below 8 KB; async-imap logs LOGIN on trace, so logger levels stay capped.
- The command names of IPC live in four places; `core/tests/contract.rs` checks them. ts-rs types are generated from
  `core/src/view.rs`; follow the repo's way of regenerating them when DTOs change.
