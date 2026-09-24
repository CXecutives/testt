"""Tests of the job-matching skill helper against the invented matching corpus.

Run: python tools/job-matching-skill/tests/test_matching.py
The work folder is built from `core/tests/fixtures/matching` (invented profile and ads);
`sample_analysis.json` is the skill's analysis of five of those ads for the senior profile.
"""

from __future__ import annotations

import copy
import datetime as dt
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
FIXTURES = ROOT / "core" / "tests" / "fixtures" / "matching"
SCRIPT = HERE.parent / "scripts" / "matching.py"
sys.path.insert(0, str(SCRIPT.parent))

import matching  # noqa: E402

# App-like findings per corpus ad (scores of the senior profile in docs/MATCHING.md).
JOBS = [
    ("K50", 100, [], ["Master's degree in Business Administration or Economics", "Fluent English"], [], []),
    ("K48", 100, [], ["Experience with Power BI"], [], []),
    ("K46", 96, ["anueRisk", "permanentRegionUnclear", "salaryUnknown"], ["Mehrjährige Erfahrung im Controlling"], [], []),
    ("K45", 89, ["permanent", "permanentRegionUnclear", "salaryUnknown"], ["Sehr gute Kenntnisse in IFRS"], [], []),
    ("K44", 85, ["permanent"], [], [], []),
    ("K52", 40, ["formalOpen"], ["Sicherer Umgang mit SAP S/4HANA"], [], ["Abgeschlossenes Studium der Informatik"]),
]


def header(path: Path) -> dict:
    lines = path.read_text(encoding="utf-8").splitlines()[:6]
    return dict(line.split(": ", 1) for line in lines)


def key_of(url: str, portal: str) -> str:
    ident = url.rstrip("/").rsplit("/", 1)[-1].split(".")[0]
    return f"{portal}:{ident}"


def build_workdir(root: Path) -> Path:
    work = root / "Job-Alert-Monitor"
    (work / "profil").mkdir(parents=True)
    txt = work / "auswertung" / "beschreibungen_txt"
    txt.mkdir(parents=True)
    shutil.copy(FIXTURES / "sample_profile_senior.json", work / "profil" / "beraterprofil.json")
    jobs = []
    for kid, score, checks, met, partial, open_ in JOBS:
        src = FIXTURES / "corpus" / f"{kid}.txt"
        h = header(src)
        name = f"20260923_{h['Quelle']}_{kid}.txt"
        shutil.copy(src, txt / name)
        portal = h["Quelle"].lower()
        jobs.append(
            {
                "key": key_of(h["Link"], portal),
                "title": h["Titel"],
                "company": h["Unternehmen"],
                "location": h["Ort"],
                "portal": portal,
                "url": h["Link"],
                "score": score,
                "band": "high" if score >= 80 else "mid" if score >= 50 else "low",
                "mustMet": 4,
                "mustTotal": 4,
                "met": met,
                "partial": partial,
                "open": open_,
                "checks": checks,
                "txtFile": name,
            }
        )
    top = {"schema": 1, "generatedAt": "2026-09-23T05:30:00.123456789Z", "rev": "e3:test", "jobs": jobs}
    (work / "auswertung" / "top_matches.json").write_text(json.dumps(top, ensure_ascii=False), encoding="utf-8")
    return work


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.work = build_workdir(self.tmp)
        self.top = matching.load_top(self.work)
        self.sample = json.loads((HERE / "sample_analysis.json").read_text(encoding="utf-8"))

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)


class Brief(Base):
    def test_default_top_five_with_text_and_findings(self):
        out = matching.brief(self.work, matching.TOP_DEFAULT)
        self.assertIn("JOB 5 ", out)
        self.assertNotIn("JOB 6 ", out)
        self.assertIn("NOT ANALYSED\n- freelancermap:4100052 Interim Lead Finanzsysteme", out)
        self.assertIn("Master's degree in Business Administration or Economics", out)
        self.assertIn("checks: anueRisk || permanentRegionUnclear || salaryUnknown", out)
        self.assertIn("Abgerufen am: 23.09.2026 07:30", out)
        self.assertNotIn("Titel: ", out, "the header block comes from the JSON")

    def test_profile_is_compact_and_without_contact_data(self):
        out = matching.brief(self.work, 10)
        self.assertIn("zielprofil_min_jahre: 10", out)
        self.assertIn("festanstellung_remote_min: 60", out)
        self.assertIn("Diplom-Kauffrau (Univ.)", out)
        self.assertIn("Interim Management | jahre 14 | auch Interim-Mandate, Interim CFO", out)
        self.assertNotIn("example.invalid", out)
        self.assertNotIn("+49", out)

    def test_the_shared_contact_filter_cases_hold(self):
        # The same file core/src/export/personal.rs checks: one filter for app and skill.
        cases = json.loads((HERE / "personal_cases.json").read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(cases), 2)
        for case in cases:
            self.assertEqual(matching.scrub_profile(case["input"]), case["expected"], case["about"])

    def test_a_tricky_profile_leaks_nothing_into_the_brief(self):
        tricky = json.loads((HERE / "personal_cases.json").read_text(encoding="utf-8"))[0]["input"]
        (self.work / "profil" / "beraterprofil.json").write_text(
            json.dumps(tricky, ensure_ascii=False), encoding="utf-8"
        )
        out = matching.brief(self.work, 1)
        for secret in ("Erika", "example.org", "171 1234567", "+49", "linkedin.com/in", "Musterweg", "1970"):
            self.assertNotIn(secret, out)
        self.assertIn("tagessatz_wunsch: 1200", out)

    def test_top_is_clamped_and_inner_lines_stay(self):
        out = matching.brief(self.work, 50)
        self.assertIn("JOB 6 ", out)
        self.assertNotIn("NOT ANALYSED", out)
        self.assertIn("Ort: 68159 Mannheim // Vertragsart: Freiberuflich", out)

    def test_the_file_the_app_writes_is_read(self):
        # Written by core/src/export/top_matches.rs (test the_skill_reads_what_the_app_writes).
        app_file = HERE / "app_top_matches.json"
        shutil.copy(app_file, self.work / "auswertung" / "top_matches.json")
        out = matching.brief(self.work, matching.TOP_DEFAULT)
        self.assertNotIn("NOTE unknown schema", out)
        self.assertIn("TOP FILE schema 2", out)
        self.assertIn("JOB 1 key freelancermap:2801", out)
        self.assertIn("stage: saved", out)
        self.assertIn("first seen: 2026-09-20T07:30:00Z", out)
        self.assertIn("open: Power BI", out)

    def test_text_file_name_cannot_leave_the_text_folder(self):
        self.assertIsNone(matching.txt_path(self.work, "../../profil/beraterprofil.json"))
        self.assertIsNone(matching.txt_path(self.work, None))


class Workdir(Base):
    def test_found_from_inside_given_or_default(self):
        inner = self.work / "auswertung" / "beschreibungen_txt"
        self.assertEqual(matching.find_workdir(None, cwd=inner, home=self.tmp / "x"), self.work.resolve())
        self.assertEqual(matching.find_workdir(str(self.work / "auswertung" / "top_matches.json")), self.work.resolve())
        # Below the current folder (before a second copy exists).
        self.assertEqual(matching.find_workdir(None, cwd=self.tmp, home=self.tmp / "x"), self.work.resolve())
        home = self.tmp / "home"
        (home / "Documents").mkdir(parents=True)
        shutil.copytree(self.work, home / "Documents" / "Job-Alert-Monitor")
        empty = self.tmp / "empty"
        empty.mkdir()
        found = matching.find_workdir(None, cwd=empty, home=home)
        self.assertEqual(found, home / "Documents" / "Job-Alert-Monitor")
        with self.assertRaises(SystemExit):
            matching.find_workdir(None, cwd=empty, home=self.tmp / "x")


class Render(Base):
    WHEN = dt.datetime(2026, 9, 24, 9, 0)

    def run_render(self, data, fmt="html"):
        path = self.tmp / "data.json"
        path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
        buf = io.StringIO()
        with redirect_stdout(buf):
            out = matching.render(path, self.work, fmt, None, self.WHEN)
        return out, buf.getvalue()

    def test_sample_passes_and_is_ranked(self):
        path, printed = self.run_render(self.sample)
        self.assertEqual(path, self.work / "auswertung" / "beschreibungen_matching" / "20260924_0900_matching.html")
        self.assertIn("CHECK OK", printed)
        doc = path.read_text(encoding="utf-8")
        order = [doc.index(t) for t in ("Head of Group Reporting", "Senior Finance Manager", "Senior Manager Group", "Lead Finanzsysteme", "Senior Controller (m/w/d)")]
        self.assertEqual(order, sorted(order), "score, then interim before an unclear contract")
        self.assertIn("Vorauswahl der App vom 23.09.2026 07:30", doc)
        self.assertIn("In der Bewerbung betonen", doc)
        self.assertIn("Master&#x27;s degree", doc)
        self.assertNotIn("Nicht weiter verfolgen", doc)

    def test_markdown_and_exclusion(self):
        data = copy.deepcopy(self.sample)
        data["jobs"] = [j for j in data["jobs"] if j["key"] != "linkedin:4100000046"]
        data["excluded"] = [
            {"key": "linkedin:4100000046", "reason": "Überlassung ausdrücklich genannt", "quote": "im Rahmen der Arbeitnehmerüberlassung"}
        ]
        path, _ = self.run_render(data, "md")
        doc = path.read_text(encoding="utf-8")
        self.assertTrue(doc.startswith("# Top 4 vom 24.09.2026"))
        self.assertIn("## Nicht weiter verfolgen", doc)
        self.assertIn("„im Rahmen der Arbeitnehmerüberlassung“", doc)

    def errors(self, mutate):
        data = copy.deepcopy(self.sample)
        mutate(data)
        return "\n".join(matching.check(data, self.top))

    def job(self, data, key):
        return next(j for j in data["jobs"] if j["key"] == key)

    def test_rubric_caps(self):
        self.assertEqual(self.errors(lambda d: None), "")
        self.assertIn("formal duty open", self.errors(lambda d: self.job(d, "freelancermap:4100052").update(score=6)))
        self.assertIn("a must is open", self.errors(lambda d: self.job(d, "linkedin:4100000050")["requirements"][0].update(status="open")))
        self.assertIn("a must is open", self.errors(lambda d: self.job(d, "linkedin:4100000046").update(score=7)))
        self.assertIn("nothing open, score 3", self.errors(lambda d: self.job(d, "linkedin:4100000048").update(score=3)))

        def half_open(d):
            for r in self.job(d, "linkedin:4100000045")["requirements"][:2]:
                r["status"] = "open"
            self.job(d, "linkedin:4100000045")["score"] = 5

        self.assertIn("half of the musts open", self.errors(half_open))
        # The location of an interim role is information only.
        self.assertEqual(self.errors(lambda d: self.job(d, "linkedin:4100000048")["frame"]["location"].update(status="open")), "")

    def test_exclusions_keys_and_text_rule(self):
        self.assertIn("needs a quote", self.errors(lambda d: d["excluded"].append({"key": "linkedin:4100000044", "reason": "Gehalt zu niedrig", "quote": ""})))
        self.assertIn("not in top_matches.json", self.errors(lambda d: self.job(d, "linkedin:4100000048").update(key="linkedin:1")))
        self.assertIn("listed twice", self.errors(lambda d: d["jobs"].append(copy.deepcopy(d["jobs"][0]))))
        self.assertIn("frame pay needs a status", self.errors(lambda d: self.job(d, "linkedin:4100000048")["frame"].pop("pay")))
        self.assertIn("dash, colon", self.errors(lambda d: self.job(d, "linkedin:4100000048").update(verdict="Gute Passung – bewerben")))
        self.assertIn("dash, colon", self.errors(lambda d: self.job(d, "linkedin:4100000048").update(verdict="Fazit: bewerben")))
        self.assertIn("dash, colon", self.errors(lambda d: self.job(d, "linkedin:4100000048").update(verdict="Bewerben!")))
        quoted = lambda d: self.job(d, "linkedin:4100000048")["frame"]["pay"].update(note="Laut „Day rate: 1,100 EUR“ über dem Minimum")
        self.assertEqual(self.errors(quoted), "")

    def test_failed_check_writes_nothing(self):
        data = copy.deepcopy(self.sample)
        self.job(data, "freelancermap:4100052")["score"] = 9
        with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
            self.run_render(data)
        self.assertFalse((self.work / "auswertung" / "beschreibungen_matching").exists())


class Cli(Base):
    def test_brief_from_the_command_line(self):
        out = subprocess.run(
            [sys.executable, str(SCRIPT), "brief", str(self.work), "--top", "2"],
            capture_output=True, encoding="utf-8", check=True,
        ).stdout
        self.assertIn("analyse the first 2", out)
        self.assertIn("Interim Head of Group Reporting", out)


if __name__ == "__main__":
    unittest.main()
