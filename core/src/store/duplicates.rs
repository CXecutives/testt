//! Cross-portal duplicates: the same job announced by two portals. A job whose full text
//! arrives is compared with the jobs of the other portals that already have one: the same
//! normalised title and company, and a text whose SimHash-64 differs in at most
//! [`MAX_DISTANCE`] bits. The later one points to the earlier one (`dup_of`); the list shows
//! one row with the other portals in `alsoOn`, and only that row is scored.

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};

use super::{Store, bump};
use crate::error::Result;
use crate::portal::{JobKey, Portal};

/// Most differing bits of two `SimHash` values of the same text.
pub const MAX_DISTANCE: u32 = 3;

impl Store {
    /// Compares a job that just got its full text with the other portals' jobs and marks
    /// it as a duplicate of the earliest match. Returns that job. A job the user marked (a
    /// favourite, moved out of the inbox, "fits anyway") is never linked: a duplicate leaves
    /// every list, and her marks must not leave with it.
    pub fn link_duplicate(&self, key: &JobKey) -> Result<Option<JobKey>> {
        self.write(|conn| {
            let Some((title, company, text)) = conn
                .query_row(
                    "SELECT title, company, desc_text FROM job
                     WHERE portal = ?1 AND job_id = ?2 AND desc_status = 'ok'
                       AND dup_of IS NULL AND desc_text IS NOT NULL
                       AND app_status IS NULL AND archived_at IS NULL AND trashed_at IS NULL
                       AND override_include IS NULL",
                    params![key.portal.key(), key.id],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
            else {
                return Ok(None);
            };
            let same = identity(&title, &company);
            if same.0.is_empty() {
                return Ok(None);
            }
            // Titles and companies first (small), texts only for the few that match.
            let mut stmt = conn.prepare_cached(
                "SELECT portal, job_id, title, company FROM job
                 WHERE portal <> ?1 AND desc_status = 'ok' AND dup_of IS NULL
                 ORDER BY first_seen_at, portal, job_id",
            )?;
            let candidates: Vec<(String, String)> = stmt
                .query_map([key.portal.key()], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
                .filter_map(std::result::Result::ok)
                .filter(|(_, _, t, c)| identity(t, c) == same)
                .map(|(portal, id, _, _)| (portal, id))
                .collect();
            let hash = simhash(&text);
            for (portal, id) in candidates {
                let other: Option<String> = conn
                    .query_row(
                        "SELECT desc_text FROM job WHERE portal = ?1 AND job_id = ?2",
                        params![portal, id],
                        |r| r.get(0),
                    )
                    .optional()?
                    .flatten();
                let Some(other) = other else { continue };
                if distance(hash, simhash(&other)) > MAX_DISTANCE {
                    continue;
                }
                let Some(portal) = Portal::from_key(&portal) else {
                    continue;
                };
                let original = JobKey { portal, id };
                conn.execute(
                    // The row goes into the original's: its own score is gone for good
                    // (never scored again, never listed on its own).
                    "UPDATE job SET dup_of = ?3, match_score = NULL, match_status = NULL,
                                    match_note = NULL, match_at = NULL, match_rev = NULL
                     WHERE portal = ?1 AND job_id = ?2",
                    params![key.portal.key(), key.id, original.to_string()],
                )?;
                bump(conn)?;
                return Ok(Some(original));
            }
            Ok(None)
        })
    }

    /// The job this one duplicates, if any.
    pub fn dup_of(&self, key: &JobKey) -> Result<Option<JobKey>> {
        let text: Option<String> = self
            .conn()
            .query_row(
                "SELECT dup_of FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(text.as_deref().and_then(JobKey::parse))
    }

    /// Per job: the other portals that announced the same job (`alsoOn`), for the given
    /// jobs; jobs without duplicates are left out.
    pub fn also_on(&self, keys: &[&JobKey]) -> Result<BTreeMap<JobKey, Vec<Portal>>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT dup_of, portal FROM job WHERE dup_of IS NOT NULL ORDER BY portal",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out: BTreeMap<JobKey, Vec<Portal>> = BTreeMap::new();
        for row in rows {
            let (original, portal) = row?;
            let (Some(original), Some(portal)) =
                (JobKey::parse(&original), Portal::from_key(&portal))
            else {
                continue;
            };
            if keys.contains(&&original) {
                let portals = out.entry(original).or_default();
                if !portals.contains(&portal) {
                    portals.push(portal);
                }
            }
        }
        Ok(out)
    }
}

/// Normalised title and company: lower case, words only, without gender markers
/// ("(m/w/d)") and legal forms ("GmbH") - the same job reads alike on every portal.
pub(crate) fn identity(title: &str, company: &str) -> (String, String) {
    // German and English job ad words, do not translate.
    const GENDER: &[&str] = &["m", "w", "d", "f", "x", "div", "all", "genders", "gn"];
    const LEGAL: &[&str] = &[
        "gmbh",
        "ag",
        "se",
        "kg",
        "kgaa",
        "co",
        "mbh",
        "ug",
        "ev",
        "e",
        "v",
        "inc",
        "ltd",
        "llc",
        "plc",
        "sa",
        "bv",
        "nv",
        "haftungsbeschränkt",
    ];
    let words = |text: &str, skip: &[&str]| -> String {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty() && !skip.contains(w))
            .collect::<Vec<_>>()
            .join(" ")
    };
    (words(title, GENDER), words(company, LEGAL))
}

/// SimHash-64 over the words of the text (lower case, two and more characters, each
/// occurrence counts) - a few changed words move only a few bits.
pub(crate) fn simhash(text: &str) -> u64 {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .collect();
    let mut weights = [0i64; 64];
    for word in words {
        let hash = fnv1a(word);
        for (bit, weight) in weights.iter_mut().enumerate() {
            if hash >> bit & 1 == 1 {
                *weight += 1;
            } else {
                *weight -= 1;
            }
        }
    }
    weights
        .iter()
        .enumerate()
        .filter(|(_, weight)| **weight > 0)
        .fold(0, |hash, (bit, _)| hash | 1 << bit)
}

/// Number of differing bits.
pub(crate) fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// FNV-1a of a word (stable across runs and versions, unlike the std hasher).
fn fnv1a(word: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in word.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "Für unseren Kunden suchen wir einen erfahrenen SAP FI/CO Berater. \
        Aufgaben: Einführung von S/4HANA Finance, Abstimmung mit den Fachbereichen, Konzeption \
        der Hauptbuchhaltung und Anlagenbuchhaltung, Schulung der Key User. Profil: mehrjährige \
        Projekterfahrung im Controlling, sehr gute Deutschkenntnisse, Reisebereitschaft.";

    #[test]
    fn the_same_job_reads_alike() {
        assert_eq!(
            identity("SAP FI/CO Berater (m/w/d)", "Ferrum Systems SE"),
            identity("SAP FI/CO-Berater (w/m/d)", "Ferrum Systems")
        );
        assert_ne!(
            identity("SAP FI/CO Berater (m/w/d)", "Ferrum Systems SE"),
            identity("SAP MM Berater (m/w/d)", "Ferrum Systems SE")
        );
    }

    #[test]
    fn near_identical_texts_are_close_and_others_far() {
        let same = simhash(TEXT);
        assert_eq!(same, simhash(&TEXT.to_uppercase()), "case does not count");
        let edited = TEXT.replace("Reisebereitschaft", "Reisebereitschaft (20 %)");
        assert!(distance(same, simhash(&edited)) <= MAX_DISTANCE);
        let other = "Wir suchen eine Projektleitung für den Rollout eines Warenwirtschaftssystems \
            in 40 Filialen. Aufgaben: Planung, Steuerung der Dienstleister, Berichtswesen an \
            die Geschäftsführung. Profil: Erfahrung im Handel und in agilen Methoden.";
        assert!(distance(same, simhash(other)) > MAX_DISTANCE);
    }

    /// A job scored before it turned out to be a duplicate: its score goes, and the top
    /// lists show the job once (as the original).
    #[test]
    fn a_duplicate_loses_its_score_and_shows_once() {
        use crate::model::{MatchRecord, MatchStatus};
        use crate::store::test_support::{mail, now, posting};

        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let first = posting(
            "https://www.freelancermap.de/nproj/12345.html",
            "SAP FI/CO Berater (m/w/d)",
            "Ferrum Systems SE",
            "Hamburg",
        );
        let second = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "SAP FI/CO Berater (m/w/d)",
            "Ferrum Systems SE",
            "Hamburg",
        );
        let record = MatchRecord {
            status: MatchStatus::Scored,
            score: 80,
            note: None,
            must_met: 1,
            must_total: 1,
            top: Vec::new(),
            facts: crate::model::KeyFacts::default(),
        };
        for p in [&first, &second] {
            store.upsert_posting(run, p, mail(), now()).unwrap();
            store
                .record_text(&p.key, TEXT, false, false, now())
                .unwrap();
            store
                .save_matches(&[(p.key.clone(), record.clone())], "r1", now())
                .unwrap();
        }
        assert_eq!(
            store.link_duplicate(&second.key).unwrap(),
            Some(first.key.clone())
        );
        let dup = store.job(&second.key).unwrap().unwrap();
        assert_eq!((dup.match_, dup.match_rev), (None, None), "no stale score");
        // The reader's fresh score (compare and set on "never scored") skips it too.
        assert!(
            !store
                .save_match_if(&second.key, &record, "r2", None, now())
                .unwrap()
        );
        let dup = store.job(&second.key).unwrap().unwrap();
        assert_eq!((dup.match_, dup.match_rev), (None, None), "still unscored");
        let keys = |jobs: Vec<crate::store::JobRow>| -> Vec<JobKey> {
            jobs.into_iter().map(|j| j.key).collect()
        };
        let top = keys(
            store
                .skill_matches(crate::store::new_since(now()), 10)
                .unwrap(),
        );
        assert_eq!(top, std::slice::from_ref(&first.key));
        let (overview, pinned) = store.overview_jobs(run).unwrap();
        assert!(!pinned);
        assert_eq!(keys(overview), [first.key]);
    }

    /// A job the user marked stays a job of its own: linked as a duplicate it would leave
    /// every list with its favourite, its place or "fits anyway".
    #[test]
    fn a_marked_job_is_never_hidden_as_a_duplicate() {
        use crate::model::Place;
        use crate::store::test_support::{mail, now, posting};
        type Mark<'a> = &'a dyn Fn(&JobKey);

        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let original = posting(
            "https://www.freelancermap.de/nproj/12345.html",
            "SAP FI/CO Berater (m/w/d)",
            "Ferrum Systems SE",
            "Hamburg",
        );
        store.upsert_posting(run, &original, mail(), now()).unwrap();
        store
            .record_text(&original.key, TEXT, false, false, now())
            .unwrap();
        let marks: [(&str, Mark<'_>); 4] = [
            ("4000000001", &|key| {
                store.set_override(key, true).unwrap();
            }),
            ("4000000002", &|key| {
                store
                    .move_jobs(std::slice::from_ref(key), Place::Trash, now())
                    .unwrap();
            }),
            ("4000000003", &|key| {
                store
                    .move_jobs(std::slice::from_ref(key), Place::Archive, now())
                    .unwrap();
            }),
            ("4000000004", &|key| {
                store.set_pinned(key, true, now()).unwrap();
            }),
        ];
        for (id, mark) in marks {
            let later = posting(
                &format!("https://www.linkedin.com/jobs/view/{id}/"),
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &later, mail(), now()).unwrap();
            mark(&later.key);
            store
                .record_text(&later.key, TEXT, false, false, now())
                .unwrap();
            assert_eq!(store.link_duplicate(&later.key).unwrap(), None, "{id}");
            assert_eq!(store.dup_of(&later.key).unwrap(), None);
        }
        // An unmarked one is linked as before.
        let plain = posting(
            "https://www.linkedin.com/jobs/view/4000000005/",
            "SAP FI/CO Berater (m/w/d)",
            "Ferrum Systems SE",
            "Hamburg",
        );
        store.upsert_posting(run, &plain, mail(), now()).unwrap();
        store
            .record_text(&plain.key, TEXT, false, false, now())
            .unwrap();
        assert_eq!(
            store.link_duplicate(&plain.key).unwrap(),
            Some(original.key)
        );
    }
}
