//! The 35 tests of the old Python engine (`ca9a2cd^:tests/test_matcher.py`), ported with
//! their exact numbers against the Rust port `jobalert_core::matching::legacy`.
//!
//! One documented deviation: `test_rate_parsing` expects `_int_rate("1.200,50") == 1200`
//! (the old engine returned 120050 because it glued the cents onto the integer part).
//!
//! The file-based tests go through the same steps as the old app: the description is
//! normalised and written as a TXT file with header lines (`write_job_text`), read back
//! (`parse_job_file`, at least 100 characters) and scored; `run_match` reads a profile file
//! and a folder like the old entry point.

use std::path::{Path, PathBuf};

use jobalert_core::matching::legacy::{
    self, LegacyMode, LegacyOutcome, LegacyScore, Term, analyze_job, build_terms, canonical_phrase,
    canonical_tokens, extract_requirements, int_rate, parse_job_file, section_kind, term_hits,
};
use serde_json::{Value, json};

fn profile() -> Value {
    json!({
        "name": "Arndt Beispiel",
        "email": "arndt@example.com",
        "skills": ["SAP S/4HANA", "IFRS-Reporting", "Controlling", "Interim Management"],
        "profil": "Erfahrener Interim-Manager für Finance und SAP, Restrukturierung.",
        "erfahrung": [
            {"firma": "Muster GmbH", "rolle": "Leiter Finanzen",
             "beschreibung": "Einführung von SAP S/4HANA, Aufbau Reporting."}
        ]
    })
}

const JOB_REQ: &str = "Ihre Aufgaben:\n- Steuerung des Monatsabschlusses\n- Aufbau eines Reportings\n\n\
Anforderungen:\n- Mehrjährige Erfahrung in SAP S/4HANA\n- Fundierte Kenntnisse im IFRS-Reporting\n\
- Erfahrung im Projektmanagement\n\nWünschenswert:\n- Kenntnisse in Power BI\n";

const JOB_UX: &str = "Ihre Aufgaben:\n- Betreuung des Webshops\n\n\
Anforderungen:\n- Mehrjährige Erfahrung im UX-Design\n- Kenntnisse in Conversion-Optimierung\n";

const JOB_CUE: &str = "Wir suchen Verstärkung für unser Team. Vorausgesetzt werden gute \
Kenntnisse in SAP S/4HANA. Eine Zertifizierung ist von Vorteil.";

const JOB_VOCAB: &str = "Unser Kunde betreibt SAP S/4HANA und Power BI im Reporting. \
Das Team sitzt in Köln und arbeitet gerne zusammen.";

const JOB_EMPTY: &str = "Willkommen in unserem Unternehmen. Wir freuen uns auf Ihre Bewerbung \
und auf eine gute Zusammenarbeit.";

// --- the old app's file round trip --------------------------------------------------

/// Python `descriptions.normalize`.
fn normalize(text: &str) -> String {
    let mut cleaned: Vec<String> = Vec::new();
    let mut blanks = 0;
    for line in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut line = line.trim().to_owned();
        while line.contains("  ") || line.contains('\t') {
            line = line.replace('\t', " ").replace("  ", " ");
        }
        if line.is_empty() {
            blanks += 1;
            if blanks <= 1 && !cleaned.is_empty() {
                cleaned.push(String::new());
            }
            continue;
        }
        blanks = 0;
        cleaned.push(line);
    }
    while cleaned.last().is_some_and(String::is_empty) {
        cleaned.pop();
    }
    cleaned.join("\n")
}

/// Python `descriptions.write_job_text` (LinkedIn, Köln, fixed company).
fn write_job(folder: &Path, job_id: &str, title: &str, text: &str) {
    let content = format!(
        "Titel: {title}\nUnternehmen: Firma GmbH\nOrt: Köln\nQuelle: LinkedIn\n\
Link: https://www.linkedin.com/jobs/view/{job_id}/\nAbgerufen am: 02.09.2026 08:00\n\n{}\n",
        normalize(text)
    );
    std::fs::write(
        folder.join(format!("20260902_LinkedIn_{job_id}.txt")),
        content,
    )
    .unwrap();
}

#[derive(Debug)]
struct Meta {
    files: usize,
    jobs: usize,
    matched: usize,
}

#[derive(Debug)]
struct Row {
    title: String,
    score: LegacyScore,
}

#[derive(Debug)]
struct ProfileError;

/// Python `run_match(profile_path, folder)`.
fn run_match(profile_path: &Path, folder: &Path) -> Result<(Meta, Vec<Row>), ProfileError> {
    let bytes = std::fs::read(profile_path).map_err(|_| ProfileError)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ProfileError)?;
    let data: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| ProfileError)?;
    if !data.is_object() {
        return Err(ProfileError);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e == "txt"));
    files.sort();
    let mut jobs = 0;
    let mut rows = Vec::new();
    for path in &files {
        let Some(job) = parse_job_file(&std::fs::read_to_string(path).unwrap()) else {
            continue;
        };
        jobs += 1;
        match legacy::legacy_outcome(&data, &job.text) {
            LegacyOutcome::Scored(score) => rows.push(Row {
                title: job.title,
                score,
            }),
            LegacyOutcome::Absent => {}
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    rows.sort_by(|a, b| {
        let ratio = |s: &LegacyScore| s.pct;
        ratio(&b.score).cmp(&ratio(&a.score))
    });
    let matched = rows.iter().filter(|r| r.score.term_hits > 0).count();
    Ok((
        Meta {
            files: files.len(),
            jobs,
            matched,
        },
        rows,
    ))
}

struct Project {
    _temp: tempfile::TempDir,
    root: PathBuf,
    desc: PathBuf,
}

impl Project {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let desc = root.join("beschreibungen_txt");
        std::fs::create_dir(&desc).unwrap();
        Self {
            _temp: temp,
            root,
            desc,
        }
    }

    fn profile_path(&self, data: &Value) -> PathBuf {
        let path = self.root.join("profil.json");
        std::fs::write(&path, data.to_string()).unwrap();
        path
    }

    fn run(&self, data: &Value) -> (Meta, Vec<Row>) {
        run_match(&self.profile_path(data), &self.desc).expect("valid profile")
    }
}

fn any_contains(list: &[String], needle: &str) -> bool {
    list.iter().any(|item| item.contains(needle))
}

// --- TermTests -----------------------------------------------------------------------

#[test]
fn test_skills_become_phrases_and_words() {
    let terms = build_terms(&profile());
    let phrases: Vec<&str> = terms
        .iter()
        .filter(|t| t.phrase)
        .map(|t| t.display.as_str())
        .collect();
    assert!(phrases.contains(&"SAP S/4HANA"));
    assert!(phrases.contains(&"Interim Management"));
    let words: Vec<&str> = terms
        .iter()
        .filter(|t| !t.phrase)
        .map(|t| t.display.as_str())
        .collect();
    assert!(words.contains(&"s/4hana"));
    assert!(words.contains(&"ifrs-reporting"));
    assert!(words.contains(&"controlling"));
    assert!(words.contains(&"restrukturierung"));
}

#[test]
fn test_personal_fields_are_ignored() {
    let terms = build_terms(
        &json!({"name": "Arndt", "email": "arndt@example.com", "skills": ["SAP FI/CO"]}),
    );
    let displays: Vec<&str> = terms.iter().map(|t| t.display.as_str()).collect();
    assert!(!displays.contains(&"arndt"));
    assert!(displays.contains(&"SAP FI/CO"));
}

#[test]
fn test_core_phrases_are_marked() {
    let terms = build_terms(&profile());
    let core_phrases: Vec<&Term> = terms.iter().filter(|t| t.core && t.phrase).collect();
    assert!(!core_phrases.is_empty());
    assert!(
        core_phrases
            .iter()
            .all(|t| t.base().to_bits() == 3.0_f64.to_bits())
    );
}

#[test]
fn test_short_keys_survive_length_filter() {
    let terms = build_terms(&json!({"skills": ["Power BI", "KI"]}));
    let keys: Vec<String> = terms.iter().map(Term::match_key).collect();
    assert!(keys.contains(&"power bi".to_owned()));
    assert!(keys.contains(&"ki".to_owned()));
}

// --- SynonymTests --------------------------------------------------------------------

fn phrase_term(display: &str) -> Term {
    Term {
        display: display.to_owned(),
        phrase: true,
        base_halves: 6,
        core: false,
        key: None,
    }
}

#[test]
fn test_word_synonym_s4hana() {
    let text = "Wir suchen einen Berater für SAP S/4HANA. ".repeat(6);
    let job_tokens = canonical_tokens(&text);
    let phrase = canonical_phrase("SAP S4 HANA");
    assert_eq!(phrase, "sap s/4hana");
    assert!(job_tokens.contains(&"s/4hana".to_owned()));
    assert_eq!(term_hits(&job_tokens, &phrase_term(&phrase)), 1);
}

#[test]
fn test_pair_synonym_project_management() {
    let text = "Gesucht wird Verstärkung im Projektmanagement. ".repeat(6);
    let job_tokens = canonical_tokens(&text);
    assert!(job_tokens.contains(&"projektmanagement".to_owned()));
    let phrase = canonical_phrase("Project Management");
    assert_eq!(phrase, "projektmanagement");
    assert_eq!(term_hits(&job_tokens, &phrase_term(&phrase)), 1);
}

// --- RequirementExtractionTests ------------------------------------------------------

fn phrases_of(reqs: &[(String, &'static str)], kind: &str) -> Vec<String> {
    reqs.iter()
        .filter(|(_, k)| *k == kind)
        .map(|(p, _)| p.clone())
        .collect()
}

#[test]
fn test_sections_classified() {
    assert_eq!(section_kind("Anforderungen:"), Some("must"));
    assert_eq!(section_kind("Ihr Profil"), Some("must"));
    assert_eq!(section_kind("Wünschenswert:"), Some("nice"));
    assert_eq!(section_kind("Von Vorteil"), Some("nice"));
    assert_eq!(section_kind("Ihre Aufgaben:"), Some("task"));
    assert_eq!(section_kind("Über uns"), Some("neutral"));
    assert_eq!(section_kind("Mehrjährige Erfahrung in SAP S/4HANA"), None);
}

#[test]
fn test_requirements_collected_from_sections() {
    let (reqs, skills) = extract_requirements(JOB_REQ).unwrap();
    let must = phrases_of(&reqs, "must");
    let nice = phrases_of(&reqs, "nice");
    assert_eq!(must.len(), 3);
    assert!(any_contains(&must, "SAP S/4HANA"));
    assert!(any_contains(&must, "IFRS-Reporting"));
    assert!(any_contains(&must, "Projektmanagement"));
    assert_eq!(nice.len(), 1);
    assert!(any_contains(&nice, "Power BI"));
    assert!(skills.is_empty());
    assert!(!any_contains(&[must, nice].concat(), "Monatsabschluss"));
}

#[test]
fn test_cue_sentences_fallback() {
    let (reqs, skills) = extract_requirements(JOB_CUE).unwrap();
    assert!(reqs.iter().any(|(_, k)| *k == "must"));
    assert!(reqs.iter().any(|(_, k)| *k == "nice"));
    let must = phrases_of(&reqs, "must");
    assert!(any_contains(&must, "SAP S/4HANA"));
    assert!(!any_contains(&must, "Zertifizierung"));
    assert!(skills.is_empty());
}

#[test]
fn test_vocab_fallback_last_resort() {
    let (reqs, skills) = extract_requirements(JOB_VOCAB).unwrap();
    assert!(reqs.is_empty());
    for skill in ["sap s/4hana", "power bi", "reporting"] {
        assert!(skills.contains(&skill.to_owned()), "{skill}");
    }
}

#[test]
fn test_unassessable_text_yields_nothing() {
    let (reqs, skills) = extract_requirements(JOB_EMPTY).unwrap();
    assert!(reqs.is_empty());
    assert!(skills.is_empty());
}

// --- MatchTests ----------------------------------------------------------------------

fn skills(list: &[&str]) -> Value {
    json!({ "skills": list })
}

#[test]
fn test_partial_coverage_scores_proportionally() {
    let p = Project::new();
    write_job(&p.desc, "1000001", "SAP Finance Berater", JOB_REQ);
    let (meta, rows) = p.run(&skills(&["SAP S/4HANA", "IFRS-Reporting"]));
    assert_eq!(meta.matched, 1);
    let row = &rows[0].score;
    assert_eq!(row.coverage, 67);
    assert_eq!(row.pct, 50);
    assert_eq!(row.mode, LegacyMode::Requirements);
    assert!(any_contains(&row.matched, "SAP S/4HANA"));
    assert!(any_contains(&row.matched, "IFRS-Reporting"));
    assert!(any_contains(&row.missing, "Projektmanagement"));
    assert!(!any_contains(&row.missing, "Power BI"));
}

#[test]
fn test_full_coverage_scores_100() {
    let p = Project::new();
    write_job(&p.desc, "1000002", "SAP Finance Berater", JOB_REQ);
    let (meta, rows) = p.run(&skills(&[
        "SAP S/4HANA",
        "IFRS-Reporting",
        "Projektmanagement",
        "Power BI",
    ]));
    assert_eq!(meta.matched, 1);
    assert_eq!(rows[0].score.pct, 100);
    assert!(rows[0].score.missing.is_empty());
}

#[test]
fn test_profile_extras_do_not_penalize() {
    let p = Project::new();
    let slim = skills(&["SAP S/4HANA", "IFRS-Reporting", "Projektmanagement"]);
    let rich = json!({
        "skills": ["SAP S/4HANA", "IFRS-Reporting", "Projektmanagement", "CRM", "Lean Six Sigma"],
        "branchen": [{"branche": "FMCG / Konsumgüter"}, {"branche": "Retail"}, {"branche": "Pharma"}]
    });
    write_job(&p.desc, "1000003", "SAP Finance Berater", JOB_REQ);
    let (_, a) = p.run(&slim);
    let (_, b) = p.run(&rich);
    assert_eq!(a[0].score.pct, b[0].score.pct);
    assert_eq!(a[0].score.coverage, b[0].score.coverage);
    assert_eq!(a[0].score.missing, b[0].score.missing);
}

#[test]
fn test_nice_to_have_is_bonus_not_penalty() {
    let p = Project::new();
    write_job(&p.desc, "1000004", "SAP Finance Berater", JOB_REQ);
    let (_, rows1) = p.run(&skills(&[
        "SAP S/4HANA",
        "IFRS-Reporting",
        "Projektmanagement",
    ]));
    let (_, rows2) = p.run(&skills(&[
        "SAP S/4HANA",
        "IFRS-Reporting",
        "Projektmanagement",
        "Power BI",
    ]));
    assert_eq!(rows1[0].score.pct, 75);
    assert_eq!(rows2[0].score.pct, 100);
}

#[test]
fn test_tasks_section_is_not_requirement() {
    let text = "Ihre Aufgaben:\n- Einführung von SAP S/4HANA\n- Betreuung der Fachbereiche im Tagesgeschäft\n\
- Dokumentation der Prozesse\n\nAnforderungen:\n- Teamfähigkeit\n- Kommunikationsstärke\n";
    let p = Project::new();
    write_job(&p.desc, "1000005", "SAP-Betrieb", text);
    let (meta, rows) = p.run(&skills(&["SAP S/4HANA"]));
    assert_eq!(meta.matched, 0);
    assert_eq!(rows.len(), 1);
    let row = &rows[0].score;
    assert_eq!(row.pct, 0);
    assert!(any_contains(&row.missing, "Teamfähigkeit"));
    assert!(!any_contains(&row.matched, "SAP"));
}

#[test]
fn test_cue_fallback_scores() {
    let p = Project::new();
    write_job(&p.desc, "1000006", "SAP Berater", JOB_CUE);
    let (meta, rows) = p.run(&skills(&["SAP S/4HANA"]));
    assert_eq!(meta.matched, 1);
    assert_eq!(rows[0].score.pct, 75);
    assert_eq!(rows[0].score.mode, LegacyMode::Requirements);
}

#[test]
fn test_vocab_fallback_scores() {
    let p = Project::new();
    write_job(&p.desc, "1000007", "Reporting-Betrieb", JOB_VOCAB);
    let (meta, rows) = p.run(&skills(&["SAP S/4HANA"]));
    assert_eq!(meta.matched, 1);
    let row = &rows[0].score;
    assert_eq!(row.mode, LegacyMode::Vocab);
    assert_eq!(row.coverage, 33);
    assert!(row.missing.contains(&"power bi".to_owned()));
    assert!(row.missing.contains(&"reporting".to_owned()));
}

#[test]
fn test_unassessable_job_produces_no_row() {
    let p = Project::new();
    write_job(&p.desc, "1000008", "Willkommen", JOB_EMPTY);
    let (meta, rows) = p.run(&profile());
    assert_eq!(meta.matched, 0);
    assert!(rows.is_empty());
}

#[test]
fn test_best_match_ranks_first() {
    let p = Project::new();
    write_job(&p.desc, "1000009", "SAP Finance Berater", JOB_REQ);
    write_job(&p.desc, "1000010", "UX Designer", JOB_UX);
    let (meta, rows) = p.run(&profile());
    assert_eq!(meta.files, 2);
    assert_eq!(meta.jobs, 2);
    assert_eq!(meta.matched, 1);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title, "SAP Finance Berater");
    assert_eq!(rows[1].title, "UX Designer");
    assert_eq!(rows[1].score.pct, 0);
    assert!(rows[0].score.pct > rows[1].score.pct);
}

#[test]
fn test_matched_and_missing_requirements_reported() {
    let p = Project::new();
    write_job(&p.desc, "1000011", "SAP Finance Berater", JOB_REQ);
    let (_, full) = p.run(&skills(&[
        "SAP S/4HANA",
        "IFRS-Reporting",
        "Projektmanagement",
        "Power BI",
    ]));
    let (_, slim) = p.run(&skills(&["SAP S/4HANA"]));
    let (full, slim) = (&full[0].score, &slim[0].score);
    assert!(full.missing.is_empty());
    assert!(any_contains(&slim.missing, "IFRS-Reporting"));
    assert!(any_contains(&slim.missing, "Projektmanagement"));
    assert!(!any_contains(&slim.missing, "Power BI"));
    assert!(slim.coverage < full.coverage);
}

#[test]
fn test_user_example_no_false_match() {
    let text = "Anforderungen:\n- Kenntnisse in Analysewerkzeugen (u. a. Python, R, VBA, Power BI) \
und praktische Erfahrung mit KI-Implementierungen im Unternehmensumfeld\n";
    let data = json!({"skills": ["SAP S/4HANA"],
        "profil": "Langjährige praktische Erfahrung mit Analysen im Unternehmensumfeld."});
    let p = Project::new();
    write_job(&p.desc, "1000012", "KI-Datenexperte", text);
    let (meta, rows) = p.run(&data);
    let row = &rows[0].score;
    assert_eq!(meta.matched, 0);
    assert!(row.matched.is_empty());
    assert_eq!(row.pct, 0);
    let open = row.missing.join(" ").to_lowercase();
    for needle in ["python", "vba", "power bi", "ki-implementierungen"] {
        assert!(open.contains(needle), "{needle}");
    }
}

#[test]
fn test_enumeration_only_covered_items_count() {
    let text = "Ihre Aufgaben:\n- Entwicklung und Pflege von Auswertungen für die Fachbereiche\n\
- Abstimmen der Anforderungen mit den Kollegen\n\nAnforderungen:\n- Kenntnisse in Python und VBA\n";
    let p = Project::new();
    write_job(&p.desc, "1000013", "Python/VBA", text);
    let (meta, rows) = p.run(&skills(&["Python"]));
    assert_eq!(meta.matched, 1);
    let row = &rows[0].score;
    assert_eq!(row.coverage, 50);
    assert_eq!(row.pct, 50);
    assert_eq!(row.matched.len(), 1);
    assert!(any_contains(&row.matched, "Python"));
    assert!(any_contains(&row.missing, "VBA"));
}

#[test]
fn test_profile_free_text_noise_never_matches() {
    let text = "Ihre Aufgaben:\n- Unterstützung der Fachbereiche bei KI-Vorhaben\n\
- Aufbereitung der Anforderungen für die Umsetzung\n\nAnforderungen:\n- Praktische Erfahrung mit KI-Implementierungen\n";
    let data = json!({"skills": ["SAP S/4HANA"],
        "profil": "Praktische Erfahrung mit KI-Implementierungen im Unternehmensumfeld und Python-Entwicklung."});
    let p = Project::new();
    write_job(&p.desc, "1000014", "KI-Projekt", text);
    let (meta, rows) = p.run(&data);
    assert_eq!(meta.matched, 0);
    assert!(rows[0].score.matched.is_empty());
    assert_eq!(rows[0].score.pct, 0);
}

#[test]
fn test_plural_tolerance_ki_implementierung() {
    let text = "Ihre Aufgaben:\n- Begleitung von KI-Vorhaben der Fachbereiche\n- Dokumentation der Ergebnisse\n\n\
Anforderungen:\n- Praktische Erfahrung mit KI-Implementierungen\n";
    let p = Project::new();
    write_job(&p.desc, "1000015", "KI-Projekt", text);
    let (meta, rows) = p.run(&skills(&["KI-Implementierung"]));
    assert_eq!(meta.matched, 1);
    assert_eq!(rows[0].score.pct, 100);
    assert!(rows[0].score.missing.is_empty());
}

#[test]
fn test_language_requirement_matches_profile_language() {
    let text = "Ihre Aufgaben:\n- Betreuung deutscher Kunden im Tagesgeschäft\n\
- Aufbereitung der monatlichen Berichte\n\nAnforderungen:\n- Verhandlungssichere Deutschkenntnisse\n";
    let p = Project::new();
    write_job(&p.desc, "1000016", "Berater (DE)", text);
    let (meta, rows) = p.run(&json!({"sprachen": [{"sprache": "Deutsch"}]}));
    assert_eq!(meta.matched, 1);
    assert_eq!(rows[0].score.pct, 100);
}

#[test]
fn test_long_free_text_requires_real_profile_phrase() {
    let text = "Anforderungen:\n- Tiefgehende Kenntnisse in der Konzernrechnungslegung nach IFRS sowie \
Erfahrung mit der Implementierung von Konsolidierungslösungen\n";
    let data = json!({"skills": ["SAP S/4HANA"], "profil": "Erfahrung mit der Implementierung von SAP-Systemen."});
    let p = Project::new();
    write_job(&p.desc, "1000017", "IFRS-Konsolidierung", text);
    let (meta, rows) = p.run(&data);
    assert_eq!(meta.matched, 0);
    assert_eq!(rows[0].score.pct, 0);
    assert!(rows[0].score.matched.is_empty());
    let (_, rows2) = p.run(&skills(&["Konzernrechnungslegung nach IFRS"]));
    assert_eq!(rows2[0].score.coverage, 50);
    assert!(any_contains(
        &rows2[0].score.matched,
        "Konzernrechnungslegung"
    ));
    assert!(any_contains(
        &rows2[0].score.missing,
        "Konsolidierungslösungen"
    ));
}

#[test]
fn test_empty_folder() {
    let p = Project::new();
    let (meta, rows) = p.run(&profile());
    assert_eq!(meta.files, 0);
    assert!(rows.is_empty());
}

#[test]
fn test_missing_profile_raises() {
    let p = Project::new();
    assert!(run_match(&p.root.join("gibt-es-nicht.json"), &p.desc).is_err());
}

#[test]
fn test_invalid_profile_raises() {
    let p = Project::new();
    let path = p.profile_path(&profile());
    std::fs::write(&path, "{kein json").unwrap();
    assert!(run_match(&path, &p.desc).is_err());
    assert_eq!(
        legacy::legacy_outcome(&json!([1]), &"x".repeat(200)),
        LegacyOutcome::InvalidProfile
    );
}

#[test]
fn test_missing_folder_is_no_error() {
    let p = Project::new();
    let (meta, rows) = run_match(&p.profile_path(&profile()), &p.root.join("leer")).unwrap();
    assert_eq!(meta.files, 0);
    assert!(rows.is_empty());
}

// --- CriteriaTests -------------------------------------------------------------------

fn criteria_profile() -> Value {
    json!({
        "skills": ["SAP S/4HANA"],
        "harte_kriterien": {
            "ausgeschlossene_vertragsarten": ["anue"],
            "laender": ["DE", "AT", "CH"],
            "min_tagessatz": 1000,
            "remote_ausserhalb_erlaubt": true,
            "verfuegbar_ab": "sofort"
        }
    })
}

fn padded(text: &str) -> String {
    format!("{text}{}", " X".repeat(30))
}

#[test]
fn test_violations_mark_and_devalue() {
    let good = padded(
        "Wir suchen einen SAP S/4HANA Berater. Tagessatz: 1.200 € pro Tag, Start: ab sofort, 100 % Remote.",
    );
    let bad = padded(
        "Wir suchen einen SAP S/4HANA Berater im Rahmen der Arbeitnehmerüberlassung. \
Tagessatz: 800 €, Start: ab November. Einsatz vor Ort in Frankfurt.",
    );
    let p = Project::new();
    write_job(&p.desc, "5000001", "Guter Job", &good);
    write_job(&p.desc, "5000002", "ANÜ-Job", &bad);
    let (_, rows) = p.run(&criteria_profile());
    let by_title = |title: &str| &rows.iter().find(|r| r.title == title).expect(title).score;
    let (bad_row, good_row) = (by_title("ANÜ-Job"), by_title("Guter Job"));
    assert!(!bad_row.violations.is_empty());
    assert!(any_contains(&bad_row.violations, "ANÜ"));
    assert!(bad_row.pct < good_row.pct);
}

#[test]
fn test_remote_outside_country_ignored() {
    let p = Project::new();
    write_job(
        &p.desc,
        "5000003",
        "Remote-US",
        &padded("SAP S/4HANA Berater gesucht, 100 % Remote, Kunde in den USA. Start: sofort."),
    );
    let (_, rows) = p.run(&criteria_profile());
    assert!(rows[0].score.violations.is_empty());
}

#[test]
fn test_onsite_outside_country_flagged() {
    let p = Project::new();
    write_job(
        &p.desc,
        "5000004",
        "Onsite-US",
        &padded("SAP S/4HANA Berater gesucht, vor Ort in den USA. Start: sofort."),
    );
    let (_, rows) = p.run(&criteria_profile());
    assert!(any_contains(&rows[0].score.violations, "US"));
}

#[test]
fn test_rate_parsing() {
    assert_eq!(int_rate("1.200"), "1200");
    // Documented deviation: the old engine returned 120050 here.
    assert_eq!(int_rate("1.200,50"), "1200");
    let signals = analyze_job("Tagessatz: 800 € pro Tag, 100 % Remote, ab sofort.");
    assert_eq!(signals.rate.as_deref(), Some("800"));
    assert_eq!(signals.remote, Some(100));
    assert!(signals.start_now);
}
