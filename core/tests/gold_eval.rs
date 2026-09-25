//! The pure parts of the gold-set examples (`core/examples/export_gold.rs`,
//! `core/examples/match_eval.rs`) on synthetic data only: file names, TXT reading, contact
//! stripping, the blind labeling file, label parsing and the metric wiring and gates.

#[path = "../examples/common/mod.rs"]
mod gold_common;

use gold_common::gold::{self, AdView, GoldJob};
use gold_common::labels;
use gold_common::metrics::Outcome::{Excluded, Scored, Unscorable};
use gold_common::metrics::{self, Exclusions, Outcome, Pair, Status};
use serde_json::json;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn job(file: &str) -> GoldJob {
    let (portal, id) = file.split_once('_').expect("portal_id");
    GoldJob {
        key: format!("{portal}:{id}"),
        file: file.to_owned(),
        portal: portal.to_owned(),
        url: format!("https://example.invalid/{id}"),
        title: "Invented role".to_owned(),
        company: "Invented GmbH".to_owned(),
        location: "Musterstadt".to_owned(),
        mail_date: Some("2026-09-16T06:40:00Z".to_owned()),
        first_seen_at: "2026-09-16T07:00:00Z".to_owned(),
        desc_status: "ok".to_owned(),
        facts: None,
    }
}

// --- gold folder -----------------------------------------------------------------------

#[test]
fn file_stem_is_a_plain_file_name() {
    assert_eq!(
        gold::file_stem("linkedin", "4012345678"),
        "linkedin_4012345678"
    );
    assert_eq!(gold::file_stem("freelance", "a/b:c d"), "freelance_a_b_c_d");
    assert_eq!(gold::file_stem("x", &"9".repeat(500)).chars().count(), 120);
}

#[test]
fn txt_header_and_body_are_read() {
    let content = "Titel: Interim CFO\nUnternehmen: Invented AG\nOrt: Musterstadt\n\
                   Quelle: Freelancermap\nLink: https://example.invalid/1\n\
                   Abgerufen am: 19.09.2026 09:00\n\nFirst line.\n\nSecond: line.\n";
    let txt = gold::parse_txt(content);
    assert_eq!(txt.title, "Interim CFO");
    assert_eq!(txt.company, "Invented AG");
    assert_eq!(txt.location, "Musterstadt");
    assert_eq!(txt.source, "Freelancermap");
    assert_eq!(txt.body, "First line.\n\nSecond: line.");
    assert_eq!(gold::parse_txt(&content.replace('\n', "\r\n")), txt);
}

#[test]
fn contact_data_is_stripped_competences_stay() {
    let profile = json!({
        "name": "Erika Muster",
        "Email": "erika@example.invalid",
        "telefon": "+49 000",
        "kontakt": { "strasse": "Weg 1" },
        "titel": "Interim CFO",
        "methoden_tools": [{ "name": "SAP S/4HANA", "email": "x@example.invalid" }],
        "harte_kriterien": { "min_tagessatz": 1000 }
    });
    let stripped = gold::strip_contact(&profile);
    assert_eq!(
        stripped,
        json!({
            "titel": "Interim CFO",
            "methoden_tools": [{ "name": "SAP S/4HANA" }],
            "harte_kriterien": { "min_tagessatz": 1000 }
        })
    );
}

#[test]
fn labeling_file_holds_only_profile_and_ad() {
    let profile = json!({ "kernkompetenzen": [{ "kompetenz": "Controlling" }] });
    let facts = json!({ "remotePercent": 100, "skills": ["Controlling", "IFRS"] });
    let ad = AdView {
        title: "Interim Controller",
        company: "Invented AG",
        location: "Musterstadt",
        portal: "Freelancermap",
        teaser: true,
        facts: Some(&facts),
        body: "Invented ad body.",
    };
    let md = gold::labeling_markdown(&profile, &ad);
    assert!(md.starts_with("# Profile\n\n```json\n{"));
    assert!(md.contains("\"kompetenz\": \"Controlling\""));
    assert!(md.contains("- Title: Interim Controller\n"));
    assert!(md.contains("- Only a short teaser of the ad was visible.\n"));
    assert!(md.contains("- Page fact remotePercent: 100\n"));
    assert!(md.contains("- Page fact skills: Controlling, IFRS\n"));
    assert!(md.ends_with("## Text\n\nInvented ad body.\n"));
    for hint in ["Link", "http", "score", "Score", "match", "Abgerufen"] {
        assert!(!md.contains(hint), "{hint}");
    }
}

#[test]
fn rubric_names_grades_exclusions_and_output() {
    for needle in [
        "- 3 = apply immediately",
        "- 2 = look closer",
        "- 1 = in the field but rather not",
        "- 0 = off-field",
        "ANÜ",
        "x 8",
        "min_jahresgehalt",
        "festanstellung_remote_min",
        "zielprofil_min_jahre",
        "\"grade\"",
        "\"excluded\"",
        "\"reason\"",
        "\"quote\"",
    ] {
        assert!(gold::RUBRIC.contains(needle), "{needle}");
    }
}

#[test]
fn jobs_json_round_trips_sorted_by_key() {
    let jobs = vec![job("linkedin_2"), job("freelance_1")];
    let text = gold::jobs_json(&jobs);
    let back: Vec<GoldJob> = serde_json::from_str(&text).expect("json");
    assert_eq!(back, vec![job("freelance_1"), job("linkedin_2")]);
    assert!(text.contains("\"mailDate\"") && text.contains("\"descStatus\""));
}

// --- labels ------------------------------------------------------------------------------

#[test]
fn labels_parse_with_defaults_and_lookups() {
    let text = r#"{
        "sample_profile_senior.json": {
            "linkedin_1": { "grade": 3, "excluded": false, "reason": "r", "quote": "q" },
            "freelance:2": { "grade": 0 },
            "linkedin_99": { "grade": 1, "excluded": true }
        },
        "sample_profile_sap": { "linkedin_1": { "grade": 2 } }
    }"#;
    let parsed = labels::parse_labels(text).expect("valid");
    let senior = labels::for_profile(&parsed, "sample_profile_senior.json").expect("by name");
    let sap = labels::for_profile(&parsed, "sample_profile_sap.json").expect("by stem");
    assert!(labels::for_profile(&parsed, "sample_profile_it.json").is_none());

    let jobs = [job("linkedin_1"), job("freelance_2")];
    let first = labels::for_job(senior, &jobs[0]).expect("by file stem");
    assert_eq!(
        (first.grade, first.excluded, first.quote.as_str()),
        (3, false, "q")
    );
    let second = labels::for_job(senior, &jobs[1]).expect("by app key");
    assert_eq!(
        (second.grade, second.excluded, second.reason.as_str()),
        (0, false, "")
    );
    assert_eq!(labels::for_job(sap, &jobs[0]).map(|l| l.grade), Some(2));
    assert_eq!(labels::unknown_jobs(senior, &jobs), 1);
}

#[test]
fn labels_reject_bad_grades_and_shapes() {
    let bad_grade = r#"{ "p.json": { "linkedin_1": { "grade": 4 } } }"#;
    let err = labels::parse_labels(bad_grade).expect_err("grade 4");
    assert!(err.contains("p.json / linkedin_1"), "{err}");
    assert!(labels::parse_labels(r#"{ "p.json": { "linkedin_1": { "grade": -1 } } }"#).is_err());
    assert!(labels::parse_labels(r#"{ "p.json": { "linkedin_1": {} } }"#).is_err());
    assert!(labels::parse_labels("[]").is_err());
}

// --- metrics -----------------------------------------------------------------------------

fn pair(new: (u8, Outcome), old: u8, grade: u8, label_excluded: bool) -> Pair {
    Pair {
        profile: 0,
        new_score: new.0,
        new_rank: 0,
        new_outcome: new.1,
        old_score: old,
        grade,
        label_excluded,
    }
}

/// A profile without any relevant job is left out of the NDCG mean; equal scores follow
/// the score before the caps.
#[test]
fn ndcg_skips_profiles_without_relevant_jobs_and_ranks_ties_by_rank() {
    let mut pairs = vec![
        pair((90, Scored), 10, 3, false),
        pair((10, Scored), 90, 0, false),
    ];
    pairs.extend([
        Pair {
            profile: 1,
            ..pair((50, Scored), 50, 0, false)
        },
        Pair {
            profile: 1,
            ..pair((40, Scored), 40, 0, true)
        },
    ]);
    assert!(close(metrics::metrics(&pairs).new.ndcg10, 1.0));
    let tie = |rank_relevant: u16, rank_other: u16| {
        let pairs = [
            Pair {
                new_rank: rank_other,
                ..pair((40, Scored), 0, 0, false)
            },
            Pair {
                new_rank: rank_relevant,
                ..pair((40, Scored), 0, 3, false)
            },
        ];
        metrics::metrics(&pairs).new.ndcg10
    };
    assert!(close(tie(420, 380), 1.0));
    assert!(tie(380, 420) < 1.0);
}

#[test]
fn keys_rank_excluded_last_and_gain_ignores_excluded_labels() {
    let scored = pair((5, Scored), 0, 3, false);
    let unscorable = pair((50, Unscorable), 0, 3, false);
    let excluded = pair((99, Excluded), 0, 3, true);
    assert!(scored.new_key() > unscorable.new_key());
    assert!(unscorable.new_key() > excluded.new_key());
    assert_eq!((scored.gain(), excluded.gain()), (3, 0));
    assert!(scored.new_buried() && unscorable.new_buried() && excluded.new_buried());
    assert!(!pair((40, Scored), 0, 3, false).new_buried());
    assert!(pair((40, Scored), 39, 3, false).old_buried());
}

#[test]
fn perfect_new_ranking_beats_reversed_old_one() {
    // New orders by grade; old orders the other way round.
    let pairs: Vec<Pair> = [
        (90, 10, 3),
        (70, 30, 2),
        (50, 50, 1),
        (30, 70, 0),
        (10, 90, 0),
    ]
    .iter()
    .map(|&(new, old, grade)| pair((new, Scored), old, grade, false))
    .collect();
    let m = metrics::metrics(&pairs);
    assert_eq!(m.n, 5);
    assert!(close(m.new.ndcg10, 1.0) && close(m.new.ndcg20, 1.0));
    assert!(m.old.ndcg10 < 0.6, "{}", m.old.ndcg10);
    // Two relevant jobs, both in every top 5: P@5 is measured against what is reachable.
    assert!(close(m.new.p5, 1.0) && close(m.old.p5, 1.0));
    assert!(
        close(m.new.spearman, 0.974_679_434_480_896_4),
        "{}",
        m.new.spearman
    );
    assert!(close(m.old.spearman, -0.974_679_434_480_896_4));
    assert_eq!(m.new.high_band, Some(1.0));
    assert_eq!(m.old.high_band, Some(0.0));
    assert_eq!((m.new.grade3_low, m.old.grade3_low), (0, 1));
    let delta = m.delta_ndcg10.expect("bootstrap");
    assert!(delta.low <= delta.mean && delta.mean <= delta.high);
    assert!(delta.mean > 0.3, "{delta:?}");
    assert_eq!(
        metrics::metrics(&pairs).delta_ndcg10,
        Some(delta),
        "fixed seed"
    );
}

#[test]
fn an_excluded_top_score_does_not_count_as_first() {
    // The engine excludes the best-scored job: it ranks last, so the grade-3 job leads.
    let pairs = [
        pair((99, Excluded), 99, 0, true),
        pair((60, Scored), 60, 3, false),
    ];
    let m = metrics::metrics(&pairs);
    assert!(close(m.new.ndcg10, 1.0));
    // The old engine has no exclusion: the job with gain 0 stays first.
    assert!(close(m.old.ndcg10, 1.0 / 3f64.log2()), "{}", m.old.ndcg10);
    assert_eq!(
        m.exclusions,
        Exclusions {
            hit: 1,
            wrong: 0,
            missed: 0
        }
    );
}

#[test]
fn exclusion_precision_and_recall() {
    let pairs = [
        pair((80, Excluded), 0, 2, true),
        pair((80, Excluded), 0, 2, false),
        pair((80, Scored), 0, 2, true),
        pair((80, Scored), 0, 2, true),
        pair((80, Scored), 0, 2, false),
    ];
    let e = Exclusions::of(&pairs);
    assert_eq!(
        e,
        Exclusions {
            hit: 1,
            wrong: 1,
            missed: 2
        }
    );
    assert_eq!(e.precision(), Some(0.5));
    assert!(close(e.recall().expect("labelled"), 1.0 / 3.0));
    assert_eq!(Exclusions::default().precision(), None);
    assert_eq!(Exclusions::default().recall(), None);
}

#[test]
fn total_averages_ndcg_over_profiles_and_pools_the_rest() {
    let mut pairs = vec![
        pair((90, Scored), 10, 3, false),
        pair((10, Scored), 90, 0, false),
    ];
    // Second profile: the new engine gets it wrong, the old one right.
    pairs.extend([
        Pair {
            profile: 1,
            ..pair((10, Scored), 90, 3, false)
        },
        Pair {
            profile: 1,
            ..pair((90, Scored), 10, 0, false)
        },
    ]);
    let total = metrics::metrics(&pairs);
    // Grade 3 at rank 2: (7 / log2 3) / 7.
    let worst = 1.0 / 3f64.log2();
    assert!(
        close(total.new.ndcg10, f64::midpoint(1.0, worst)),
        "{}",
        total.new.ndcg10
    );
    assert!(close(total.old.ndcg10, total.new.ndcg10));
    assert!(close(total.new.spearman, 0.0));
    assert_eq!(total.new.high_band, Some(0.5));
    assert_eq!(total.new.grade3_low, 1);
    // Symmetric profiles: the bootstrap difference centres on zero.
    let delta = total.delta_ndcg10.expect("bootstrap");
    assert!(
        delta.mean.abs() < 0.05 && delta.low < 0.0 && delta.high > 0.0,
        "{delta:?}"
    );
}

#[test]
fn gates_and_status() {
    let good: Vec<Pair> = (0..10u8)
        .map(|i| {
            let grade = if i < 5 { 3 } else { 0 };
            // New orders by grade, old the other way round.
            pair((100 - i * 9, Scored), 10 + i * 9, grade, false)
        })
        .collect();
    let m = metrics::metrics(&good);
    let gates = metrics::gates(&m);
    assert_eq!(gates.len(), 14);
    assert!(gates.iter().all(|g| g.pass), "{gates:?}");
    assert_eq!(metrics::status(0, &gates), Status::NoLabels);
    assert_eq!(metrics::status(59, &gates), Status::Preliminary);
    assert_eq!(metrics::status(60, &gates), Status::Pass);

    // One wrong exclusion and a buried grade-3 job fail their gates.
    let mut bad = good.clone();
    bad[0].new_outcome = Excluded;
    let gates = metrics::gates(&metrics::metrics(&bad));
    let failed: Vec<&str> = gates
        .iter()
        .filter(|g| !g.pass)
        .map(|g| g.name.as_str())
        .collect();
    assert!(failed.contains(&"Exclusion precision"), "{failed:?}");
    assert!(
        failed.contains(&"Grade-3 jobs buried (< 40, excluded or unscorable)"),
        "{failed:?}"
    );
    assert_eq!(metrics::status(60, &gates), Status::Fail);
    assert_eq!(metrics::status(59, &gates), Status::Preliminary);

    // Grade 3 below grade 0 in every pair fails the concordance gate.
    let reversed: Vec<Pair> = good
        .iter()
        .map(|p| Pair {
            new_score: 100 - p.new_score,
            ..*p
        })
        .collect();
    let gates = metrics::gates(&metrics::metrics(&reversed));
    let failed: Vec<&str> = gates
        .iter()
        .filter(|g| !g.pass)
        .map(|g| g.name.as_str())
        .collect();
    assert!(failed.contains(&"Concordance grade 0 v 3"), "{failed:?}");
}

/// The order below the top: Spearman over the relevant pairs only, and the concordance of
/// label grade and shown score per grade pair (ties half, the score of an excluded job kept).
#[test]
fn relevant_spearman_and_concordance_per_grade_pair() {
    let pairs = [
        pair((90, Scored), 90, 3, false),
        pair((70, Scored), 50, 2, false),
        pair((70, Scored), 60, 1, false),
        // Excluded by the engine with its score kept, a grade-2 job by the labels.
        pair((80, Excluded), 30, 2, false),
        pair((20, Scored), 70, 0, false),
        pair((10, Scored), 20, 0, true),
    ];
    let m = metrics::metrics(&pairs);
    let c = |lower, higher| {
        let c = m.new.concordance_of(lower, higher);
        (c.share, c.pairs)
    };
    assert_eq!(c(0, 3), (Some(1.0), 2));
    // The excluded job keeps its 80 against the grade-0 jobs.
    assert_eq!(c(0, 2), (Some(1.0), 4));
    // Grade 1 against 2: tied at 70 (half) and below 80.
    assert_eq!(c(1, 2), (Some(0.75), 2));
    assert_eq!(c(2, 3), (Some(1.0), 2));
    assert_eq!(c(1, 3), (Some(1.0), 1));
    assert_eq!(m.old.concordance_of(0, 2).share, Some(0.5));
    // Relevant pairs by list order (the excluded job last), gains 3, 2, 1, 2: key ranks
    // 4, 2.5, 2.5, 1 against gain ranks 4, 2.5, 1, 2.5.
    assert_eq!(m.new.spearman_relevant, Some(0.5));
    // Relevant pairs of one gain have no order to judge.
    let flat = [
        pair((90, Scored), 0, 3, false),
        pair((10, Scored), 0, 3, false),
    ];
    assert_eq!(metrics::metrics(&flat).new.spearman_relevant, None);
    let table = metrics::order_table(&[("a.json".to_owned(), m)], &m);
    assert_eq!(table.lines().count(), 4, "{table}");
    assert!(table.contains("| 1.000 / 0.500 (4) |"), "{table}");
    assert!(!table.contains("NaN"));
}

#[test]
fn tables_have_one_row_per_profile_and_the_total() {
    let pairs = [
        pair((90, Scored), 10, 3, false),
        pair((10, Excluded), 90, 0, true),
    ];
    let m = metrics::metrics(&pairs);
    let table = metrics::table(&[("a.json".to_owned(), m)], &m);
    assert_eq!(table.lines().count(), 4, "{table}");
    assert!(table.contains("| a.json | 2 | 1.000 / "));
    assert!(table.contains("| **Total** | 2 |"));
    assert!(!table.contains("NaN"));
    let gates = metrics::gate_table(&metrics::gates(&m));
    assert_eq!(gates.lines().count(), 16);
    assert!(gates.contains("| Exclusion precision | 1.000 | 1.000 | pass |"));
}
