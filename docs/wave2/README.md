# Wave 2: the last findings and three features

A work list for one session. Read `CLAUDE.md` and `docs/PLAN.md` first; their rules are binding. Delete this folder in
the last commit of the wave.

## Inputs
- `findings.json`: 102 findings of the ship audit on `a2b2353` (lenses: names and labels 20, live forms 18, live jobs
  17, ui-forms 12, ui-jobs 12, backend 9, export-matching 7, ui-core 6), each with files, evidence, repro, fix and
  `verification` (99 confirmed by an independent verifier, 3 uncertain) plus the verifier's notes (`betterFix` wins over
  `fix`). Paths like `<local scratch>/...` in a repro pointed to the auditor's local probe; rebuild the repro from the
  description. Wave 1, the rings, the sliding bar, the prompts and engine 14 landed after the audit (`git log`), so many
  findings are already fixed: check each against the current code first and skip the fixed ones.
- `features.json`: the features the user chose, in this order: features-01 (the reader's "Rahmen" strip always shows
  the ad's day rate and start as plain chips), features-02 (the search matches every word in any field plus the portal
  name), features-07 (remove the unreachable tile and portal filter code), features-05 (Excel columns for the exclusion
  reason, favourite, day rate, start, duration, remote share; NOT workload) last.
- `docs-edits.json`: 51 documentation edits drafted on `a2b2353`; apply the ones still true, skip the rest (MATCHING.md
  versions 12 to 14 and two plan boxes are done already).

## Two passes after the list
1. Review the shell track merge (`git log --merges --grep "shell track"`, its WIP commit `4a65b1a` was stopped
   mid-work by a usage limit): read its diff against CLAUDE.md and what it aimed at (input, platform, shell, shared
   components, the Windows scrollbar width, Shift+Arrow multi-select, arrow keys scrolling Einstellungen and Profil;
   its tests are in `tools/ui-harness/specs/wave1-shell.spec.ts`), fix what is wrong or half done, with tests. The backend, rings+bar and CV prompt merges
   deserve a quick read too.
2. Text amount: read every string of `ui/src/lib/i18n/de.ts` (and its English mirror) in its place (the harness shows
   each screen, `?lang=en` for English, all stub scenarios) and cut what is too long, redundant (says what is already
   visible or repeats a heading), explains the obvious, or could go. Only what is needed, plain and human, one short
   sentence per note, buttons one verb phrase. Keep one word per thing (docs/PLAN.md glossary).

## How to work (economical, full quality)
- One item at a time, in this order: features-01, features-02, features-07; then findings by severity (high, medium,
  low), grouped by file so a file is opened once; then features-05; then the doc edits.
- For each: verify in the code, fix the best way under CLAUDE.md, add or adjust a test, keep the German catalog the
  source and mirror English, keep the UI text rules. A finding that contradicts a decision in docs/PLAN.md is skipped
  with a note.
- Gates before every push: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `npm run check`, `npm run harness -- --project chromium --project webkit`. On Linux the
  screenshot baselines (`baseline: ...` tests) differ from the Windows ones: ignore only those, never update or commit
  baseline PNGs; every other harness test must pass.
- Small English commits on a branch `wave2`; push it and open a pull request against `main`; do not merge. CI (Windows,
  macOS) is the judge for the baselines; the integrator refreshes them on Windows.
- If the budget runs short, push what is done and list the rest in the pull request.
