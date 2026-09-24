# Job-Alert-Monitor

Desktop app (Tauri 2 + Rust, Windows and macOS) that reads job alert mails (LinkedIn, freelancermap,
freelance.de) from Gmail, fetches the job pages, **scores every job against a consultant profile**, and
writes Excel, TXT (contract with the external `job-matching` skill) and an HTML overview.
Rebuild of `CXecutives/projektscraper` (read-only reference; old engine at git ref `ca9a2cd^`, branch `legacy-python`).
**Progress, phases and decisions live in `docs/PLAN.md` - read it before any work and tick its boxes.**

## Hard rules
- Never modify `CXecutives/projektscraper`. Push only to `origin` = `CXecutives/testt`, fast-forward only; never push
  `legacy-python`; never bypass `.githooks/pre-push`; never force-push.
- Secrets: never ask for, read, log, print or commit passwords. The Gmail app password lives only in the OS keychain.
  The repo is public: no real mails, profiles or labels outside `core/tests/fixtures/private/` (ignored).
- Everything in the repo is English (code, comments, docs, logs, errors, tests, CI, commits). Exceptions are the
  modules marked `external contract - do not translate` (TXT header, folder names, profile JSON keys, German mail
  patterns, matching lexicon) and the German UI catalog `ui/src/lib/i18n/de.ts`. `core/tests/language.rs` enforces it.
- The TXT files stay byte-identical (`header_is_exactly_the_contract`, `txt_is_blind_to_the_match`).
- UI: light mode only, no theme infrastructure. It must feel like a native app: the native window frame of the OS on
  both (Windows: its title bar in the app's colours via DWM in `platform.rs`; no title bar drawn in the page); the
  content inside the window is identical, and it differs between Windows and macOS only where the OS convention does
  (listed in docs/PLAN.md "Platforms", decided in `ui/src/lib/platform.ts` and `src-tauri/src/platform.rs` only).
  Buttons and controls react to the left click only; scroll areas also scroll with the middle mouse button
  (autoscroll where the OS offers it); text a user would want to copy (ad text, job title, company, facts, profile
  values, paths) is marked `data-copy`, selectable and copies with Ctrl/Cmd+C; everything else is not selectable.
  Keys inside fields: Tab, Enter, Esc, Ctrl/Cmd+C/V/X/A/Z. No browser context menu, no zoom, no reload. All input
  handling in `ui/src/lib/input/input.ts`. Motion is quick (<= 180 ms, ring fill <= 400 ms), ease-out, no bounce.
- Styling only via tokens in `ui/src/styles/tokens.css`; controls only from `ui/src/components/`; Tauri only via
  `ui/src/lib/ipc/api.ts`; motion only via `ui/src/lib/motion/`. At most one primary button per view. Lint enforces it.
- UI text (German): little text, only what is needed, plain and human. No AI-style writing: no dashes or
  em dashes as separators, no colons in labels or headings, no "X: Y" constructions, no filler, no exclamation
  marks, no emoji. Buttons are one verb phrase, notes one short sentence. The UI must not look AI-generated
  (no sparkles, no gradient text, no glow for decoration).
- The backend never sends prose: notices, errors and status are `{code, params}`; texts live in the UI catalog.
- Scraping stays conservative: only links from the user's alert mails, every request through `admit` (policy.json),
  stop on 429/999/403/captcha/login wall, never bypass captchas or 2FA. Every portal can be switched off.
- Run events stay below 8 KB (Tauri channel messages above 8 KB bypass the ACL). async-imap logs LOGIN on trace:
  logger levels stay capped.

## Architecture
- `core/` (`jobalert-core`, `#![forbid(unsafe_code)]`, no UI prose): `mail/` IMAP scan · `portal/` adapters + registry ·
  `fetch/` queue, HTTP, policy, health · `matching/` pure integer scoring engine · `store/` SQLite (schema chain) ·
  `pipeline/` runs (scan → fetch → score → export) · `export/` xlsx, txt, overview html · `view.rs` IPC DTOs (ts-rs).
- `src-tauri/`: `main.rs` start, `platform.rs` (only place with per-OS code), `session.rs` (freelance.de webview),
  `commands/` (IPC), `smoke.rs` (debug-only smoke probe). Command names live in 4 places; `core/tests/contract.rs` checks.
- `ui/`: Svelte 5 + Vite + TypeScript. `styles/`, `components/` (design system), `features/` (screens), `lib/`.
- `tools/`: `ui-harness/` (Playwright, Chromium + WebKit), `eval/legacy_baseline.py`, `icon.py`, `third-party.mjs`,
  `job-matching-skill/` (optional stage-2 Claude skill for the top matches; `python tools/job-matching-skill/tests/test_matching.py`).

## Commands
- `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace`
- `npm ci` · `npm run check` (svelte-check, eslint, stylelint, prettier) · `npm run harness` · `npm run build`
- `npx tauri build` (release bundles) · debug smoke: `target/debug/job-alert-monitor --dry-run --smoke --smoke-run`

## Workflow
- Small English commits, each green on its own. Tick `docs/PLAN.md`. Parallel tracks work in their own worktree with
  disjoint files; shared files (`Cargo.toml`, `package.json`, `core/src/lib.rs`, `core/src/view.rs`,
  `src-tauri/src/commands/**`, `build.rs`, `capabilities/main.json`, `tauri*.conf.json`) belong to the integrator.
- Remove worktrees right after merging and run `cargo clean` there (disk).
- macOS is verified through the `macos-latest` CI runner (real app screenshots, dmg install probe) and WebKit locally.
