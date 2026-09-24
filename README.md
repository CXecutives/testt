# Job-Alert-Monitor

Desktop app (Tauri 2 + Rust, Windows and macOS) that turns job alert mails into a scored,
searchable list of postings.

## What it does

Reads job alert mails from LinkedIn, freelancermap and freelance.de out of a Gmail mailbox
(read-only IMAP; the Gmail app password is stored only in the OS keychain). Fetches each
job's full posting page politely: every portal can be switched off individually, and each
has its own request limits and pauses. Scores every job locally against a consultant
profile with an explainable, integer-only rule engine — nothing is sent to the cloud.
Writes an Excel workbook (`JobAlerts.xlsx`), one TXT file per job and an HTML overview into
a work folder. An optional stage-2 Claude skill in `tools/job-matching-skill/` can re-rank
the app's top matches.

## Install

**Windows:** run the NSIS installer (current-user install, no admin rights needed). The
build is unsigned, so Windows SmartScreen blocks it on first launch — choose
*More info -> Run anyway*.

**macOS:** open the `.dmg` (Apple Silicon only, macOS 14 or newer). On first start, go to
*System Settings -> Privacy & Security -> Open Anyway*. The build is only ad-hoc signed, so
the keychain also asks for permission again after every update.

Builds are not published as GitHub releases; download the current installer/dmg from the CI
workflow's artifacts.

## First start

Connect the Gmail mailbox with an app password, then choose an existing profile or create
one from the template, then press **Abrufen** (fetch).

## The profile

The consultant profile is a JSON file with German keys, in the structure shown by
`core/tests/fixtures/matching/sample_profile_senior.json` (name, years of experience,
education, career stations, competences, and hard criteria such as minimum day rate or
allowed countries). A competence entry can carry an `auch` list of alternative terms that
count toward it. Everything personal about the consultant lives only in this file, never in
the app itself or its code.

## How matching works

The engine reads the posting text (must/nice/other sections, inline headings, requirement
sentences), splits requirements into AND/OR items, and matches each item against the
profile's competences and their `auch` aliases through a stemming/compound-aware ladder.
Hard criteria (ANÜ, country, day rate, availability, and — for permanent roles — minimum
salary and region) are evaluated separately and can exclude a job outright. The final score
combines must/nice coverage with a relevance-weighted shrinkage estimate; jobs are banded
into high/mid/low, or marked excluded or unscorable. Details, formulas and the full
parameter set are in `docs/MATCHING.md`.

Measured against the old engine on the 52-job corpus, across four test profiles (in-band =
score inside the pre-agreed band, distance = sum of band-distance errors, Spearman =
rank correlation of score vs. band):

| Profile | Jobs | In band, old | In band, new | Distance, old | Distance, new | Spearman, old | Spearman, new |
|---|---|---|---|---|---|---|---|
| fin (interim finance) | 52 | 24/52 | 49/52 | 561 | 14 | 0.60 | 0.87 |
| it (SAP/IT project lead) | 52 | 41/52 | 51/52 | 193 | 5 | 0.60 | 0.84 |
| senior (interim CFO) | 52 | 20/52 | 49/52 | 549 | 14 | 0.62 | 0.75 |
| sap (SAP FI/CO consultant) | 52 | 35/52 | 51/52 | 224 | 6 | 0.68 | 0.89 |

## Privacy and portals

Everything runs locally: jobs, full texts, the database and logs never leave the machine
except for the IMAP connection to Gmail, the HTTP requests to the job portals themselves,
and — only if the user runs the optional stage-2 skill — the user's own Claude, without an
API key. The mailbox is opened read-only: mails stay unread, and nothing is changed, deleted
or sent. No passwords are stored by the app except the Gmail app password, which lives in
the OS keychain (Windows Credential Manager / macOS Keychain).

Portal notes: LinkedIn postings are fetched through its public guest view without a
LinkedIn account, which sits in a grey zone under LinkedIn's terms of use. freelancermap
postings are fetched through its public project pages. freelance.de sign-in is off by
default, because freelance.de's crawling policy does not clearly permit signed-in scraping;
enabling it is a manual per-portal choice with an account-risk warning in the UI. All
portals are fetched conservatively — request limits, randomized pauses, and an automatic
stop on rate limits, blocks or captchas, never bypassed.

## Development

**Requirements:** the Rust toolchain pinned in `rust-toolchain.toml`, Node 24, npm.

**Commands:**
```
npm ci
npm run check          # svelte-check, eslint, stylelint, prettier
npm run harness         # Playwright UI harness (Chromium + WebKit)
npm run build           # builds the UI into ui/dist
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx tauri build         # release bundles (NSIS on Windows, app/dmg on macOS), pinned Tauri CLI
```

**Project layout:**
- `core/` - `jobalert-core`: mail scanning, portal fetching, the matching engine, storage
  and export; no UI code, no `unsafe`.
- `src-tauri/` - the thin app layer: Tauri commands, window and platform glue.
- `ui/` - Svelte 5 + Vite + TypeScript frontend.
- `tools/` - UI harness, evaluation scripts, icon generation, the optional
  `job-matching-skill/`.
- `docs/` - `PLAN.md` (project plan and decisions) and `MATCHING.md` (matching engine
  reference and measurements).

**Dry run:** `--dry-run` starts the app without touching the real mailbox, portals or work
folder, for trying out or screenshotting the UI.
