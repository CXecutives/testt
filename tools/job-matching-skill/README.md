# job-matching skill (optional stage 2)

A Claude skill that takes the app's best matches after a fetch and checks them in depth against
the consultant profile: requirement by requirement with quotes from the ad, contract type, rate
or salary, region for permanent roles, seniority and formal duties. It writes a short German
report with a score from 1 to 10 and what to emphasise in an application.

It is optional. The app scores every job itself (stage 1, `docs/MATCHING.md`) and needs nothing
from the skill. The skill runs in the user's own Claude (no API key, it counts against the
user's plan) and reads only the app's top 5 ads, or up to 10 on request, so it stays cheap.

## What it needs

- A fetch in the app with a profile, so the work folder holds `auswertung/top_matches.json`,
  `auswertung/beschreibungen_txt/` and `profil/beraterprofil.json`.
- Claude with access to that work folder: the Claude desktop app with the folder connected, or
  Claude Code started in it (or pointed at it).
- Code execution for the helper script `scripts/matching.py` (Python 3.9 or newer, standard
  library only). Without it Claude reads the files directly and writes a Markdown report.

## Add it to Claude

- Claude app (desktop or web): zip the folder `job-matching-skill` (the zip holds the folder
  with `SKILL.md` inside), then open Settings, Capabilities, Skills, choose "Upload skill" and
  pick the zip. Code execution must be on. The skill is called `job-matching`: turn off or
  remove an older skill of the same name first.
- Claude Code: copy the folder to `~/.claude/skills/job-matching/` (on Windows
  `%USERPROFILE%\.claude\skills\job-matching\`).

`README.md` and `tests/` are not needed by Claude; they may stay in the zip.

## Run it

In a chat with access to the work folder say "Gleiche meine Top-Treffer ab" (for more jobs, for
example "Gleiche meine Top 8 ab"; at most 10). Claude answers with two or three sentences and
writes the report to `auswertung/beschreibungen_matching/yyyymmdd_hhmm_matching.html` in the
work folder. The app leaves that folder alone.

## Changes against the original skill

- No fixed personal path: the work folder is named by the user or found through
  `auswertung/top_matches.json` (Windows and macOS).
- No screening of every TXT file with sub-agents: the skill starts from the app's findings
  (`met`, `partial`, `open`, `checks`) and reads only the top N ads.
- Every threshold comes from the profile; the rubric wording follows the app's engine (unclear
  contract types and a missing remote share are checks, exclusions only with a quote; tools are
  no formal duty).
- The report adds the quotes behind each gap and the points to emphasise; excluded top matches
  are listed with their quote at the end.

## Test

```
python tools/job-matching-skill/tests/test_matching.py
```

It builds a work folder from the invented corpus in `core/tests/fixtures/matching` and checks
the brief, the folder search, the rubric caps and both report formats
(`tests/sample_analysis.json` is the skill's analysis of five corpus ads for the senior profile).
