"""Freeze the outputs of the old Python matching engine (git ref ca9a2cd^).

The old engine (matcher.py plus the modules it imports) is read with `git show`
into a temporary folder and imported from there - the old code is never checked
out. Every job file is scored on its own through the old entry point
`matcher.run_match(profile, folder)`, so a crash in one file (the old engine
raises IndexError on inline headings such as "Anforderungen: ...") is recorded
for that job instead of aborting the run.

Run from the repository root with Python 3.14:

  py -3.14 tools/eval/legacy_baseline.py corpus     # -> core/tests/fixtures/matching/legacy.json
  py -3.14 tools/eval/legacy_baseline.py edge       # -> core/tests/fixtures/matching/legacy_edge.json
  py -3.14 tools/eval/legacy_baseline.py lexicon    # -> core/tests/fixtures/matching/legacy_lexicon.json
  py -3.14 tools/eval/legacy_baseline.py unicode    # -> core/src/matching/python_unicode.txt
  py -3.14 tools/eval/legacy_baseline.py run --corpus DIR --profile P.json [--profile Q.json] --out FILE

`corpus` and `edge` refuse to change an existing file (the frozen baseline must
never change); `--check` recomputes and compares instead of writing.
"""

from __future__ import annotations

import argparse
import datetime as dt
import importlib
import json
import platform
import shutil
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

SOURCE_REF = "ca9a2cd^"
OLD_FILES = ("matcher.py", "descriptions.py", "profile_store.py", "config.py")
REPO = Path(__file__).resolve().parents[2]
FIXTURES = REPO / "core" / "tests" / "fixtures" / "matching"
PROFILES = (FIXTURES / "sample_profile.json", FIXTURES / "sample_profile_it.json")


def git(*args: str) -> bytes:
    return subprocess.run(["git", *args], cwd=REPO, check=True, capture_output=True).stdout


def load_old_engine(target: Path):
    """Write the old modules into `target` and import matcher from there."""
    for name in OLD_FILES:
        (target / name).write_bytes(git("show", f"{SOURCE_REF}:{name}"))
    sys.path.insert(0, str(target))
    for name in ("matcher", "descriptions", "profile_store", "config"):
        sys.modules.pop(name, None)
    return importlib.import_module("matcher")


def score_file(matcher, profile: Path, job_file: Path, scratch: Path) -> dict:
    """Score one job file through the old entry point."""
    folder = scratch / job_file.stem
    folder.mkdir()
    shutil.copyfile(job_file, folder / job_file.name)
    try:
        _meta, rows = matcher.run_match(profile, folder)
    except Exception as exc:  # noqa: BLE001 - the crash itself is the recorded result
        return {"pct": None, "error": f"{type(exc).__name__}: {exc}"}
    if not rows:
        return {"pct": None}
    row = rows[0]
    return {
        "pct": row["pct"],
        "coverage": row["coverage"],
        "mode": row["mode"],
        "matched": row["matched_reqs"],
        "missing": row["missing_reqs"],
        "violations": row["violations"],
        "termHits": len(row["matched_terms"]),
        "mustTotal": row["must_total"],
        "niceTotal": row["nice_total"],
        "niceCovered": row["nice_covered"],
    }


def baseline(folder: Path, profiles: list[Path], command: str) -> dict:
    jobs = sorted(folder.glob("*.txt"))
    result = {
        "command": command,
        "source": SOURCE_REF,
        "sourceCommit": git("rev-parse", SOURCE_REF).decode().strip(),
        "python": platform.python_version(),
        "date": dt.date.today().isoformat(),
        "profiles": {},
    }
    with tempfile.TemporaryDirectory() as temp:
        temp_path = Path(temp)
        engine_dir = temp_path / "engine"
        engine_dir.mkdir()
        matcher = load_old_engine(engine_dir)
        for profile in profiles:
            scratch = temp_path / f"jobs-{profile.stem}"
            scratch.mkdir()
            result["profiles"][profile.name] = {
                job.stem: score_file(matcher, profile, job, scratch) for job in jobs
            }
    return result


def write_frozen(out: Path, data: dict, check: bool, force: bool) -> int:
    """Write `data`; an existing file must stay unchanged (header fields aside)."""
    if out.exists():
        old = json.loads(out.read_text(encoding="utf-8"))
        same = old.get("profiles") == data["profiles"] and old.get("sourceCommit") == data["sourceCommit"]
        if check or not force:
            print(f"{out.relative_to(REPO)}: {'identical' if same else 'DIFFERENT'}")
            return 0 if same else 1
    elif check:
        print(f"{out} does not exist")
        return 1
    out.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out.relative_to(REPO)}")
    return 0


def lexicon() -> dict:
    with tempfile.TemporaryDirectory() as temp:
        m = load_old_engine(Path(temp))
        return {
            "source": SOURCE_REF,
            "stopwords": sorted(m._STOPWORDS),
            "synonymWords": dict(sorted(m.SYNONYM_WORDS.items())),
            "synonymPairs": sorted([a, b, v] for (a, b), v in m.SYNONYM_PAIRS.items()),
            "mustHeadings": sorted(m._MUST_HEADINGS),
            "niceHeadings": sorted(m._NICE_HEADINGS),
            "taskHeadings": sorted(m._TASK_HEADINGS),
            "neutralHeadings": sorted(m._NEUTRAL_HEADINGS),
            "vocab": list(m._JOB_SKILL_VOCAB),
            "skillKeys": sorted(m.SKILL_KEYS),
            "personalKeys": sorted(m.PERSONAL_KEYS),
            "coreKeys": sorted(m._CORE_KEYS),
            "softKeys": sorted(m._SOFT_KEYS),
            "countryNames": dict(sorted(m._COUNTRY_NAMES.items())),
        }


def unicode_tables() -> str:
    """Python's own character classes, so the Rust port needs no Unicode guesswork."""

    def ranges(pred) -> list[tuple[int, int]]:
        out, start = [], None
        for cp in range(0x110000):
            if pred(chr(cp)):
                start = cp if start is None else start
            elif start is not None:
                out.append((start, cp - 1))
                start = None
        if start is not None:
            out.append((start, 0x10FFFF))
        return out

    lines = [
        f"# Generated by tools/eval/legacy_baseline.py unicode (Python {platform.python_version()},"
        f" Unicode {unicodedata.unidata_version}). Do not edit.",
        "# word: non-ASCII ranges of Python re \\w (str.isalnum() or '_')",
    ]
    lines += [f"w {a:x} {b:x}" for a, b in ranges(lambda c: c > "\x7f" and (c.isalnum() or c == "_"))]
    lines.append("# digit: ranges of Python re \\d (str.isdecimal()); each run counts 0-9 from its start")
    lines += [f"d {a:x} {b:x}" for a, b in ranges(str.isdecimal)]
    lines.append("# fold: str.casefold() for every code point it changes")
    for cp in range(0x110000):
        folded = chr(cp).casefold()
        if folded != chr(cp):
            lines.append(f"f {cp:x} " + " ".join(f"{ord(c):x}" for c in folded))
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=("corpus", "edge", "lexicon", "unicode", "run"))
    parser.add_argument("--corpus", type=Path, help="folder with job TXT files (run)")
    parser.add_argument("--profile", type=Path, action="append", help="profile JSON (run, repeatable)")
    parser.add_argument("--out", type=Path, help="output file (run)")
    parser.add_argument("--check", action="store_true", help="compare with the frozen file instead of writing")
    parser.add_argument("--force", action="store_true", help="overwrite a frozen file (never for legacy.json)")
    args = parser.parse_args()
    command = "py -3.14 tools/eval/legacy_baseline.py " + args.command

    if args.command == "corpus":
        data = baseline(FIXTURES / "corpus", list(PROFILES), command)
        return write_frozen(FIXTURES / "legacy.json", data, args.check, args.force)
    if args.command == "edge":
        edge_profiles = [*PROFILES, FIXTURES / "legacy_edge_profile.json"]
        data = baseline(FIXTURES / "legacy_edge", edge_profiles, command)
        return write_frozen(FIXTURES / "legacy_edge.json", data, args.check, args.force)
    if args.command == "lexicon":
        out = FIXTURES / "legacy_lexicon.json"
        out.write_text(json.dumps(lexicon(), ensure_ascii=False, indent=1) + "\n", encoding="utf-8", newline="\n")
        print(f"wrote {out.relative_to(REPO)}")
        return 0
    if args.command == "unicode":
        out = REPO / "core" / "src" / "matching" / "python_unicode.txt"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(unicode_tables(), encoding="utf-8", newline="\n")
        print(f"wrote {out.relative_to(REPO)}")
        return 0
    if not (args.corpus and args.profile and args.out):
        parser.error("run needs --corpus, --profile and --out")
    data = baseline(args.corpus, args.profile, command + f" --corpus {args.corpus}")
    args.out.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
