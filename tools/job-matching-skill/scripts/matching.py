#!/usr/bin/env python3
"""Helper of the `job-matching` skill (optional stage 2 of the Job-Alert-Monitor).

Two steps, standard library only, Windows and macOS:

  python matching.py brief  [WORK_FOLDER] [--top N]
      Prints one compact brief for Claude: the profile without personal data, then for each
      of the app's top N jobs (default 5, at most 10) the app's findings and the ad text.

  python matching.py render DATA.json [WORK_FOLDER] [--format html|md] [--out DIR]
      Checks Claude's analysis against the rubric and writes the German report
      `auswertung/beschreibungen_matching/yyyymmdd_hhmm_matching.html` (or `.md`).
      Prints `CHECK OK <path>` or `CHECK FAILED` with the reasons.

Without WORK_FOLDER the folder is searched: the current folder and its parents, then the
app's default `Documents/Job-Alert-Monitor` in the home folder, then three levels below
the current folder. Folder and file names are the app's contract (German by design).
"""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import os
import re
import sys
from pathlib import Path

RESULT_DIR = "auswertung"
TOP_FILE = "top_matches.json"
TXT_DIR = "beschreibungen_txt"
REPORT_DIR = "beschreibungen_matching"
PROFILE = Path("profil") / "beraterprofil.json"
TOP_DEFAULT, TOP_MAX = 5, 10
TEXT_LIMIT = 12000
HEADER_KEYS = ("Titel:", "Unternehmen:", "Ort:", "Quelle:", "Link:", "Abgerufen am:")

# The contact-data filter the app's AI prompts use too (one file, the same cases in both
# test suites): contact data, the consultant's name and testimonial prose stay out.
PERSONAL_DATA = Path(__file__).resolve().parent.parent / "personal_data.json"
STRING_LIMIT = 240

REQ_WEIGHTS = ("must", "nice", "formal")
REQ_STATUS = ("met", "partial", "open")
FRAME_KEYS = ("contract", "pay", "seniority", "availability", "location")
FRAME_STATUS = ("met", "partial", "open", "info")
CONTRACTS = ("interim", "permanent", "unclear")


# --------------------------------------------------------------------------- work folder


def _has_top(folder: Path) -> bool:
    return (folder / RESULT_DIR / TOP_FILE).is_file()


def find_workdir(given: str | None, cwd: Path | None = None, home: Path | None = None) -> Path:
    """The work folder: the given path (folder, `auswertung` or the JSON file) or a search."""
    if given:
        p = Path(given).expanduser().resolve()
        for cand in (p, p.parent, p.parent.parent):
            if _has_top(cand):
                return cand
        raise SystemExit(f"No {RESULT_DIR}/{TOP_FILE} in or above {p}.")
    cwd = (cwd or Path.cwd()).resolve()
    home = home or Path.home()
    for cand in (cwd, *cwd.parents):
        if _has_top(cand):
            return cand
    docs = ("Documents", "Dokumente", "OneDrive/Documents", "OneDrive/Dokumente")
    for sub in docs:
        cand = home / sub / "Job-Alert-Monitor"
        if _has_top(cand):
            return cand
    found = sorted(
        (p.parent.parent for p in _shallow(cwd, 4) if p.name == TOP_FILE and p.parent.name == RESULT_DIR),
        key=lambda f: (f / RESULT_DIR / TOP_FILE).stat().st_mtime,
        reverse=True,
    )
    if found:
        return found[0]
    raise SystemExit(
        f"No {RESULT_DIR}/{TOP_FILE} found. Ask the user for the app's work folder "
        "(the app shows it in its settings) and pass it as WORK_FOLDER."
    )


def _shallow(root: Path, depth: int):
    """Files at most `depth` levels below `root`, skipping hidden and heavy folders."""
    skip = {"node_modules", "target", ".git", "AppData", "Library"}
    for base, dirs, files in os.walk(root):
        level = len(Path(base).relative_to(root).parts)
        dirs[:] = [d for d in dirs if not d.startswith(".") and d not in skip and level < depth]
        for f in files:
            yield Path(base) / f


# --------------------------------------------------------------------------- reading


def load_json(path: Path):
    with open(path, encoding="utf-8-sig") as f:
        return json.load(f)


def load_top(work: Path) -> dict:
    top = load_json(work / RESULT_DIR / TOP_FILE)
    if not isinstance(top, dict) or not isinstance(top.get("jobs"), list):
        raise SystemExit(f"{TOP_FILE} has no job list.")
    return top


def txt_path(work: Path, name: str | None) -> Path | None:
    """The job's text file; only the bare file name is used (no path from the JSON)."""
    if not name:
        return None
    p = work / RESULT_DIR / TXT_DIR / Path(name.replace("\\", "/")).name
    return p if p.is_file() else None


def ad_text(path: Path) -> str:
    """The ad without the leading header block (the JSON already has it), except the date."""
    lines = path.read_text(encoding="utf-8-sig", errors="replace").splitlines()
    fetched, i = "", 0
    while i < len(lines) and lines[i].startswith(HEADER_KEYS):
        if lines[i].startswith("Abgerufen am:"):
            fetched = lines[i]
        i += 1
    body = re.sub(r"\n{3,}", "\n\n", "\n".join(l.rstrip() for l in lines[i:])).strip()
    if len(body) > TEXT_LIMIT:
        body = body[:TEXT_LIMIT].rstrip() + f"\n[truncated after {TEXT_LIMIT} characters]"
    return (fetched + "\n" if fetched else "") + body


def _short(value) -> str:
    s = " ".join(str(value).split())
    return s if len(s) <= STRING_LIMIT else s[: STRING_LIMIT - 3].rstrip() + "..."


_RULES: dict | None = None
_UMLAUTS = {"ä": "ae", "ö": "oe", "ü": "ue", "ß": "ss"}


def _rules() -> dict:
    global _RULES
    if _RULES is None:
        rules = json.loads(PERSONAL_DATA.read_text(encoding="utf-8"))
        rules["patterns"] = [re.compile(p) for p in rules["valuePatterns"]]
        _RULES = rules
    return _RULES


def _normalise(key: str) -> str:
    """Lower case, umlauts spelled out, every other character an underscore."""
    out: list[str] = []
    for c in key.strip().lower():
        if c in _UMLAUTS:
            out.append(_UMLAUTS[c])
        elif c.isascii() and c.isalnum():
            out.append(c)
        elif out and out[-1] != "_":
            out.append("_")
    return "".join(out).rstrip("_")


def _personal_key(key: str, place: str) -> bool:
    r = _rules()
    k = _normalise(key)
    return (
        any(p in k for p in r["keyParts"])
        or any(t in r["keyTokens"] for t in k.split("_"))
        or any(p in k for p in r["proseParts"])
        or (place != "inside" and k in r["nameKeys"])
    )


def _personal_section(key: str) -> bool:
    k = "_" + _normalise(key)
    return any("_" + s in k for s in _rules()["personalSections"])


def scrub_text(text: str) -> str:
    """A text without mail addresses, links and phone numbers (spaces collapsed where one went)."""
    changed = False
    for pattern in _rules()["patterns"]:
        if pattern.search(text):
            text = pattern.sub("", text)
            changed = True
    return " ".join(text.split()) if changed else text.strip()


def _scrub(value, place: str):
    if isinstance(value, dict):
        return {
            k: _scrub(v, "personal" if place == "personal" or _personal_section(k) else "inside")
            for k, v in value.items()
            if not _personal_key(k, place)
        }
    if isinstance(value, list):
        return [_scrub(v, place) for v in value]
    if isinstance(value, str):
        return scrub_text(value)
    return value


def scrub_profile(profile):
    """The profile without contact data, the consultant's name and testimonial prose."""
    return _scrub(profile, "top" if isinstance(profile, dict) else "inside")


def _item(value) -> str:
    """One list entry on one line: scalars joined, nested lists comma-separated."""
    if isinstance(value, dict):
        parts = []
        for k, v in value.items():
            if v in (None, "", [], {}):
                continue
            if isinstance(v, list):
                parts.append(f"{k} " + ", ".join(_item(x) for x in v))
            elif isinstance(v, dict):
                parts.append(f"{k} {{{_item(v)}}}")
            else:
                parts.append(_short(v) if not parts else f"{k} {_short(v)}")
        return " | ".join(parts)
    if isinstance(value, list):
        return ", ".join(_item(x) for x in value)
    return _short(value)


def profile_lines(profile: dict) -> list[str]:
    """The profile for the brief, after the shared contact-data filter."""
    out = []
    for key, value in scrub_profile(profile).items():
        if value in (None, "", [], {}):
            continue
        if isinstance(value, list) and any(isinstance(x, dict) for x in value):
            out.append(f"{key}:")
            out += [f"  - {_item(x)}" for x in value]
        elif isinstance(value, dict):
            out.append(f"{key}:")
            out += [f"  {k}: {_item(v)}" for k, v in value.items()]
        else:
            out.append(f"{key}: {_item(value)}")
    return out


def brief(work: Path, top_n: int) -> str:
    top = load_top(work)
    jobs = top["jobs"]
    if not jobs:
        raise SystemExit(
            f"{TOP_FILE} lists no scored jobs. The user should fetch jobs in the app with a "
            "profile first."
        )
    prof_path = work / PROFILE
    if not prof_path.is_file():
        raise SystemExit(f"No profile at {PROFILE.as_posix()} in the work folder.")
    n = max(1, min(top_n, TOP_MAX, len(jobs)))
    out = [
        f"WORK FOLDER {work}",
        f"TOP FILE schema {top.get('schema')} generated {top.get('generatedAt')} rev {top.get('rev')}"
        f" jobs {len(jobs)}, analyse the first {n}",
    ]
    if top.get("schema") not in (1, 2):
        out.append("NOTE unknown schema, read fields with care")
    out += ["", "PROFILE (personal data left out)"] + profile_lines(load_json(prof_path))
    for i, job in enumerate(jobs[:n], 1):
        out += ["", "=" * 72, f"JOB {i} key {job.get('key')}"]
        for f in ("title", "company", "location", "portal", "url"):
            out.append(f"{f}: {job.get(f, '')}")
        out.append(
            f"app score {job.get('score')} ({job.get('band')}), musts met "
            f"{job.get('mustMet')}/{job.get('mustTotal')}"
        )
        # Schema 2: the user's stage (saved, applied, ...) and when the app first saw the job.
        if job.get("appStatus"):
            out.append(f"stage: {job.get('appStatus')}")
        if job.get("firstSeenAt"):
            out.append(f"first seen: {job.get('firstSeenAt')}")
        for f in ("met", "partial", "open", "checks"):
            vals = job.get(f) or []
            out.append(f"{f}: " + (" || ".join(map(str, vals)) if vals else "-"))
        p = txt_path(work, job.get("txtFile"))
        out.append("TEXT")
        out.append(ad_text(p) if p else "(no text file: judge from the app's findings, say so)")
    rest = jobs[n:]
    if rest:
        out += ["", "NOT ANALYSED"]
        out += [f"- {j.get('key')} {j.get('title')} ({j.get('score')})" for j in rest]
    return "\n".join(out)


# --------------------------------------------------------------------------- checking

_SEPARATOR = re.compile(r"\s[–—-]\s|—|!|:\s")
_QUOTED = re.compile(r"„[^“”\"]*[“”\"]|\"[^\"]*\"")
_EMOJI = re.compile("[\U0001F300-\U0001FAFF☀-➿]")


def _style(text: str, where: str, errors: list[str]) -> None:
    """German text rule of the app: no dash or colon separators, no exclamation marks, no emoji."""
    text = _QUOTED.sub("", text)  # quotes from the ad are data
    if _SEPARATOR.search(text) or _EMOJI.search(text):
        errors.append(f"{where}: dash, colon, exclamation mark or emoji in {text!r}")


def check(data: dict, top: dict) -> list[str]:
    errors: list[str] = []
    by_key = {j.get("key"): j for j in top["jobs"]}
    jobs = data.get("jobs") or []
    excluded = data.get("excluded") or []
    seen: set[str] = set()
    for entry in list(jobs) + list(excluded):
        key = entry.get("key")
        if key not in by_key:
            errors.append(f"{key}: not in {TOP_FILE}")
        if key in seen:
            errors.append(f"{key}: listed twice")
        seen.add(key)
    if len(jobs) > TOP_MAX:
        errors.append(f"{len(jobs)} jobs, at most {TOP_MAX}")
    if not jobs:
        errors.append("no analysed job left to show")
    for ex in excluded:
        k = ex.get("key")
        if not str(ex.get("quote", "")).strip():
            errors.append(f"{k}: an exclusion needs a quote from the ad")
        if not str(ex.get("reason", "")).strip():
            errors.append(f"{k}: an exclusion needs a reason")
        else:
            _style(ex["reason"], f"{k} reason", errors)
    for job in jobs:
        errors += _check_job(job)
    return errors


def _check_job(job: dict) -> list[str]:
    k = job.get("key")
    errors: list[str] = []
    score = job.get("score")
    if not isinstance(score, int) or not 2 <= score <= 10:
        errors.append(f"{k}: score must be a whole number from 2 to 10 (shown jobs get at least 2)")
        score = 0
    if job.get("contract") not in CONTRACTS:
        errors.append(f"{k}: contract must be one of {CONTRACTS}")
    reqs = job.get("requirements") or []
    if not reqs:
        errors.append(f"{k}: requirement rows missing")
    for r in reqs:
        where = f"{k} {r.get('label')!r}"
        if r.get("weight") not in REQ_WEIGHTS or r.get("status") not in REQ_STATUS:
            errors.append(f"{where}: weight {REQ_WEIGHTS}, status {REQ_STATUS}")
        for f in ("label", "quote", "evidence"):
            if not str(r.get(f, "")).strip():
                errors.append(f"{where}: {f} missing")
        _style(str(r.get("label", "")), where, errors)
        _style(str(r.get("evidence", "")), f"{where} evidence", errors)
    frame = job.get("frame") or {}
    for f in FRAME_KEYS:
        row = frame.get(f)
        if not isinstance(row, dict) or row.get("status") not in FRAME_STATUS:
            errors.append(f"{k}: frame {f} needs a status from {FRAME_STATUS}")
        elif not str(row.get("note", "")).strip():
            errors.append(f"{k}: frame {f} needs a note")
        else:
            _style(row["note"], f"{k} frame {f}", errors)
    if not str(job.get("verdict", "")).strip():
        errors.append(f"{k}: verdict missing")
    else:
        _style(job["verdict"], f"{k} verdict", errors)
    emph = job.get("emphasise") or []
    if not 1 <= len(emph) <= 4:
        errors.append(f"{k}: 1 to 4 points to emphasise")
    for e in emph:
        _style(str(e), f"{k} emphasise", errors)

    # Score caps of the rubric (the same caps as the app's engine, on a 1 to 10 scale).
    musts = [r for r in reqs if r.get("weight") in ("must", "formal")]
    open_musts = [r for r in musts if r.get("status") == "open"]
    formal_open = [r for r in open_musts if r.get("weight") == "formal"]
    counted = [f for f in FRAME_KEYS if not (f == "location" and job.get("contract") == "interim")]
    frame_open = [f for f in counted if (frame.get(f) or {}).get("status") == "open"]
    any_open = bool(open_musts or frame_open)
    if formal_open and score > 4:
        errors.append(f"{k}: formal duty open, score {score} > 4")
    elif len(open_musts) >= 2 and 2 * len(open_musts) >= len(musts) and score > 4:
        errors.append(f"{k}: half of the musts open, score {score} > 4")
    elif any_open and score > 6:
        errors.append(f"{k}: a must is open, score {score} > 6")
    if len(musts) >= 2 and not any(r.get("status") in ("met", "partial") for r in musts) and score > 3:
        errors.append(f"{k}: no must met, score {score} > 3")
    if not any_open and score < 4:
        errors.append(f"{k}: nothing open, score {score} < 4")
    return errors


# --------------------------------------------------------------------------- report
# User-facing German report text (product decision, like ui/src/lib/i18n/de.ts).

DE_STATUS = {"met": "erfüllt", "partial": "teilweise", "open": "offen", "info": "Info"}
DE_FRAME = {
    "contract": "Vertragsart",
    "pay": "Vergütung",
    "seniority": "Seniorität",
    "availability": "Verfügbarkeit",
    "location": "Einsatzort",
}
DE_CONTRACT = {"interim": "Interim", "permanent": "Festanstellung", "unclear": "Vertragsart unklar"}
CONTRACT_RANK = {"interim": 0, "permanent": 1, "unclear": 2}


def _tag(r: dict) -> str:
    if r["weight"] == "formal":
        return "formale Pflicht"
    word = "fehlt" if r["status"] == "open" else "teilweise"
    return f"Kann · {word}" if r["weight"] == "nice" else word


def _counts(job: dict) -> tuple[int, int, int, int]:
    reqs = job["requirements"]
    met = sum(r["status"] == "met" for r in reqs)
    part = sum(r["status"] == "partial" for r in reqs)
    opn = sum(r["status"] == "open" for r in reqs)
    must_open = sum(r["status"] == "open" and r["weight"] != "nice" for r in reqs)
    return met, part, opn, must_open


def _sorted(jobs: list[dict], by_key: dict) -> list[dict]:
    return sorted(
        jobs,
        key=lambda j: (
            -j["score"],
            CONTRACT_RANK.get(j["contract"], 9),
            _counts(j)[3],
            -(by_key[j["key"]].get("score") or 0),
        ),
    )


def _now() -> dt.datetime:
    try:
        from zoneinfo import ZoneInfo

        return dt.datetime.now(ZoneInfo("Europe/Berlin"))
    except Exception:
        return dt.datetime.now()


def _app_time(top: dict) -> str:
    raw = re.sub(r"\.\d+", "", str(top.get("generatedAt") or ""))
    try:
        t = dt.datetime.fromisoformat(raw.replace("Z", "+00:00"))
        try:
            from zoneinfo import ZoneInfo

            t = t.astimezone(ZoneInfo("Europe/Berlin"))
        except Exception:
            t = t.astimezone()
        return t.strftime("%d.%m.%Y %H:%M")
    except ValueError:
        return ""


CSS = """
:root{--bg:#f6f7f9;--card:#fff;--ink:#1c2430;--mute:#5b6675;--line:#e3e7ec;--g:#1f8a4c;--gbg:#eaf6ef;
--y:#b8790a;--ybg:#fdf4e3;--r:#c62828;--rbg:#fdeeee;--b:#2457a6}
*{box-sizing:border-box;min-width:0}
body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.45 system-ui,-apple-system,Segoe UI,Roboto,sans-serif;padding:20px 16px 48px}
header,main{max-width:1040px;margin:0 auto}
h1{font-size:1.45rem;margin:0 0 2px}h2{font-size:1.12rem;margin:0}
.sub{color:var(--mute);font-size:.9rem;margin-bottom:18px}
.card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:16px 18px;margin:0 0 14px;overflow-wrap:anywhere}
.rank{display:flex;gap:10px;align-items:flex-start;justify-content:space-between}
a{color:var(--b);text-decoration:none}a:hover{text-decoration:underline}
.pos{color:var(--mute);font-size:.78rem;font-weight:700;letter-spacing:.08em;text-transform:uppercase}
.meta{color:var(--mute);font-size:.9rem;margin-top:2px}
.badge{flex:0 0 auto;min-width:56px;text-align:center;border-radius:10px;padding:7px 10px;color:#fff;font-weight:700;font-size:1.35rem;line-height:1}
.badge small{display:block;font-size:.62rem;font-weight:600;opacity:.92;margin-top:3px}
.s-g{background:var(--g)}.s-y{background:var(--y)}.s-r{background:var(--r)}
.emp{margin:12px 0 0;padding:10px 12px;background:#f2f5fa;border-radius:8px;font-size:.95rem}
.bar{display:flex;height:10px;border-radius:5px;overflow:hidden;margin:14px 0 6px;background:var(--line)}
.bar i{display:block}.bar .g{background:var(--g)}.bar .y{background:var(--y)}.bar .r{background:var(--r)}
.legend{display:flex;gap:14px;flex-wrap:wrap;font-size:.84rem;color:var(--mute);margin-bottom:12px}
.legend b{color:var(--ink)}
.cols{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.col{border-radius:10px;padding:11px 12px}.col.ok{background:var(--gbg)}.col.gap{background:var(--rbg)}
.col h3,.tips h3{font-size:.74rem;text-transform:uppercase;letter-spacing:.07em;margin:0 0 8px;color:var(--mute)}
.col.ok h3{color:var(--g)}.col.gap h3{color:var(--r)}
.chips{display:flex;flex-wrap:wrap;gap:5px}
.chip{background:#fff;border:1px solid #cfe6d8;color:#14623a;border-radius:999px;padding:3px 9px;font-size:.82rem;line-height:1.3}
.gaps{list-style:none;margin:0;padding:0}
.gaps li{background:#fff;border-left:4px solid var(--r);border-radius:6px;padding:6px 9px;margin-bottom:6px;font-size:.86rem}
.gaps li.tw{border-left-color:var(--y)}.gaps li.kann{border-left-color:#9aa5b1}
.gaps b,.gaps q{display:block}.gaps q{color:var(--ink);font-style:italic}.gaps span{color:var(--mute)}
.tag{float:right;margin-left:8px;font-size:.66rem;font-weight:700;letter-spacing:.05em;text-transform:uppercase;color:var(--r)}
.gaps li.tw .tag{color:var(--y)}.gaps li.kann .tag{color:#6b7480}
.none{font-size:.86rem;color:var(--mute)}
.pills{display:flex;flex-wrap:wrap;gap:6px;margin-top:12px}
.pill{border:1px solid var(--line);border-radius:8px;padding:5px 9px;font-size:.8rem;background:#fbfcfd}
.pill i{display:inline-block;width:8px;height:8px;border-radius:50%;margin-right:6px}
.st-met i{background:var(--g)}.st-partial i{background:var(--y)}.st-open i{background:var(--r)}.st-info i{background:#9aa5b1}
.pill b{font-weight:600}.pill span{color:var(--mute)}
.tips{margin-top:12px}.tips ul{margin:0;padding-left:20px;font-size:.9rem}
.out li{margin-bottom:6px;font-size:.9rem}.out q{font-style:italic}
@media (max-width:640px){body{padding:14px 12px 40px}.cols{grid-template-columns:1fr}}
"""


def render_html(data: dict, top: dict, when: dt.datetime) -> str:
    E = lambda s: html.escape("" if s is None else str(s))
    by_key = {j["key"]: j for j in top["jobs"]}
    jobs = _sorted(data["jobs"], by_key)
    cards = []
    for i, job in enumerate(jobs, 1):
        src = by_key[job["key"]]
        title = E(src.get("title"))
        if src.get("url"):
            title = f'<a href="{E(src["url"])}" target="_blank" rel="noopener">{title}</a>'
        s = job["score"]
        cls = "s-g" if s >= 8 else "s-y" if s >= 5 else "s-r"
        met, part, opn, must_open = _counts(job)
        segs = "".join(
            f'<i class="{c}" style="flex:{n}"></i>' for c, n in (("g", met), ("y", part), ("r", opn)) if n
        )
        ok = [r for r in job["requirements"] if r["status"] == "met"]
        gaps = sorted(
            (r for r in job["requirements"] if r["status"] != "met"),
            key=lambda r: (r["weight"] == "nice", r["status"] != "open"),
        )
        chips = "".join(
            f'<span class="chip" title="{E(r["quote"])} | {E(r["evidence"])}">{E(r["label"])}</span>'
            for r in ok
        ) or '<div class="none">kein Punkt voll erfüllt</div>'
        items = "".join(
            f'<li class="{"kann" if r["weight"] == "nice" else "tw" if r["status"] == "partial" else ""}">'
            f'<span class="tag">{E(_tag(r))}</span><b>{E(r["label"])}</b>'
            f'<q>{E(r["quote"])}</q><span>{E(r["evidence"])}</span></li>'
            for r in gaps
        )
        gap_html = f'<ul class="gaps">{items}</ul>' if items else '<div class="none">nichts offen</div>'
        pills = "".join(
            f'<div class="pill st-{job["frame"][f]["status"]}"><i></i><b>{DE_FRAME[f]}</b> '
            f'<span>{E(job["frame"][f]["note"])}</span></div>'
            for f in FRAME_KEYS
        )
        tips = "".join(f"<li>{E(t)}</li>" for t in job["emphasise"])
        cards.append(
            f'<article class="card"><div class="rank"><div><div class="pos">Platz {i}</div>'
            f"<h2>{title}</h2><div class=\"meta\">{E(src.get('company'))} · {E(src.get('location'))}"
            f" · {DE_CONTRACT[job['contract']]}</div></div>"
            f'<div class="badge {cls}">{s}<small>von 10</small></div></div>'
            f'<p class="emp">{E(job["verdict"])}</p>'
            f'<div class="bar">{segs}</div><div class="legend"><span><b>{met}</b> erfüllt</span>'
            f"<span><b>{part}</b> teilweise</span><span><b>{opn}</b> offen</span>"
            f"<span><b>{must_open}</b> davon Pflicht</span>"
            f"<span>Vorauswahl der App <b>{E(src.get('score'))}</b> von 100</span></div>"
            f'<div class="cols"><section class="col ok"><h3>Das passt</h3><div class="chips">{chips}</div></section>'
            f'<section class="col gap"><h3>Das fehlt oder ist offen</h3>{gap_html}</section></div>'
            f'<div class="pills">{pills}</div>'
            f'<section class="tips"><h3>In der Bewerbung betonen</h3><ul>{tips}</ul></section></article>'
        )
    out = ""
    if data.get("excluded"):
        rows = "".join(
            f'<li><a href="{E(by_key[x["key"]].get("url"))}" target="_blank" rel="noopener">'
            f'{E(by_key[x["key"]].get("title"))}</a> · {E(x["reason"])} <q>{E(x["quote"])}</q></li>'
            for x in data["excluded"]
        )
        out = f'<article class="card"><h2>Nicht weiter verfolgen</h2><ul class="out">{rows}</ul></article>'
    day = when.strftime("%d.%m.%Y")
    sub = [E(data.get("consultant") or ""), f"{len(jobs)} Anzeigen vertieft"]
    if _app_time(top):
        sub.append(f"Vorauswahl der App vom {_app_time(top)}")
    return (
        '<!doctype html><html lang="de"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width,initial-scale=1">'
        f"<title>Job-Matching {day}</title><style>{CSS}</style></head><body>"
        f"<header><h1>Top {len(jobs)} vom {day}</h1>"
        f'<div class="sub">{" · ".join(x for x in sub if x)}</div></header>'
        f"<main>{''.join(cards)}{out}</main></body></html>"
    )


def render_md(data: dict, top: dict, when: dt.datetime) -> str:
    by_key = {j["key"]: j for j in top["jobs"]}
    jobs = _sorted(data["jobs"], by_key)
    lines = [f"# Top {len(jobs)} vom {when.strftime('%d.%m.%Y')}", ""]
    if data.get("consultant"):
        lines += [f"{data['consultant']} · {len(jobs)} Anzeigen vertieft", ""]
    for i, job in enumerate(jobs, 1):
        src = by_key[job["key"]]
        met, part, opn, must_open = _counts(job)
        lines += [
            f"## Platz {i} · [{src.get('title')}]({src.get('url')}) · {job['score']} von 10",
            "",
            f"{src.get('company')} · {src.get('location')} · {DE_CONTRACT[job['contract']]} · "
            f"Vorauswahl der App {src.get('score')} von 100",
            "",
            job["verdict"],
            "",
            f"**Das passt** ({met} erfüllt)",
            "",
        ]
        lines += [f"- {r['label']} · „{r['quote']}“ · {r['evidence']}" for r in job["requirements"] if r["status"] == "met"] or ["- kein Punkt voll erfüllt"]
        lines += ["", f"**Das fehlt oder ist offen** ({part} teilweise, {opn} offen, {must_open} davon Pflicht)", ""]
        gaps = [r for r in job["requirements"] if r["status"] != "met"]
        lines += [f"- {r['label']} ({_tag(r)}) · „{r['quote']}“ · {r['evidence']}" for r in gaps] or ["- nichts offen"]
        lines += ["", "**Rahmen**", ""]
        lines += [
            f"- {DE_FRAME[f]} {DE_STATUS[job['frame'][f]['status']]} · {job['frame'][f]['note']}"
            for f in FRAME_KEYS
        ]
        lines += ["", "**In der Bewerbung betonen**", ""] + [f"- {t}" for t in job["emphasise"]] + [""]
    if data.get("excluded"):
        lines += ["## Nicht weiter verfolgen", ""]
        lines += [
            f"- [{by_key[x['key']].get('title')}]({by_key[x['key']].get('url')}) · {x['reason']} · „{x['quote']}“"
            for x in data["excluded"]
        ]
    return "\n".join(lines).rstrip() + "\n"


def render(data_path: Path, work: Path, fmt: str, out_dir: Path | None, when: dt.datetime | None = None) -> Path:
    data = load_json(data_path)
    top = load_top(work)
    errors = check(data, top)
    if errors:
        print("CHECK FAILED\n- " + "\n- ".join(errors))
        raise SystemExit(1)
    when = when or _now()
    doc = render_html(data, top, when) if fmt == "html" else render_md(data, top, when)
    folder = out_dir or work / RESULT_DIR / REPORT_DIR
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"{when.strftime('%Y%m%d_%H%M')}_matching.{fmt}"
    path.write_text(doc, encoding="utf-8")
    by_key = {j["key"]: j for j in top["jobs"]}
    jobs = _sorted(data["jobs"], by_key)
    print(f"CHECK OK {path}")
    print("Ranking: " + ", ".join(f"{by_key[j['key']].get('title')} ({j['score']})" for j in jobs))
    print(f"Excluded: {len(data.get('excluded') or [])}")
    return path


def main(argv: list[str] | None = None) -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("brief")
    b.add_argument("work", nargs="?")
    b.add_argument("--top", type=int, default=TOP_DEFAULT)
    r = sub.add_parser("render")
    r.add_argument("data")
    r.add_argument("work", nargs="?")
    r.add_argument("--format", choices=("html", "md"), default="html")
    r.add_argument("--out")
    a = ap.parse_args(argv)
    work = find_workdir(a.work)
    if a.cmd == "brief":
        print(brief(work, a.top))
    else:
        render(Path(a.data), work, a.format, Path(a.out) if a.out else None)


if __name__ == "__main__":
    main()
