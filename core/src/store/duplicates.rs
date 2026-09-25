//! Cross-portal duplicates: the same job announced by two portals. A job whose full text
//! (or guest teaser) arrives is compared with the jobs of the other portals that already
//! have one: the same normalised title and company, and a text whose SimHash-64 differs in
//! at most [`MAX_DISTANCE`] bits - or, for a teaser against a full text, whose word triples
//! the full text contains. The later one points to the earlier one (`dup_of`), a teaser
//! always to the full text; the list shows one row with the other portals in `alsoOn`, and
//! only that row is scored.

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};

use super::{Store, bump};
use crate::error::Result;
use crate::portal::{JobKey, Portal};

/// Most differing bits of two `SimHash` values of the same text.
pub const MAX_DISTANCE: u32 = 3;

impl Store {
    /// Compares a job that just got its full text (or a guest's teaser) with the other
    /// portals' jobs - and with its own portal's where one key has no portal id (a slug
    /// link of a project the alerts name by its id) - and links it to the earliest match;
    /// the row with the portal's id is always the original. Returns the original.
    ///
    /// Two full texts match by `SimHash`; a teaser (the start of the ad, cut) matches a full
    /// text when its word triples are contained in it ([`MIN_CONTAINED`]), since a
    /// 300-character teaser is never within a few bits of the whole ad. A teaser row always
    /// points to the full-text row, never the reverse, so the row that is listed and scored
    /// carries the full text: a full text that arrives after its teaser becomes the original,
    /// and the teaser (with anything that pointed to it) points to it; the original keeps what
    /// the user and the files already had of the job ([`inherit`]).
    ///
    /// A job the user marked (a favourite, moved out of the inbox, "fits anyway") is never
    /// linked: a duplicate leaves every list, and her marks must not leave with it. An
    /// archived, trashed or closed job is never the original either - the fresh, open
    /// announcement would vanish behind it.
    pub fn link_duplicate(&self, key: &JobKey) -> Result<Option<JobKey>> {
        self.write(|conn| {
            let Some(own) = row(conn, key.portal.key(), &key.id, true)? else {
                return Ok(None);
            };
            let same = identity(&own.title, &own.company);
            if same.0.is_empty() {
                return Ok(None);
            }
            // Titles and companies first (small), texts only for the few that match. The
            // other portals' jobs - and the same portal's where one side has no portal id
            // (freelancermap's `/projekt/<slug>` link of a project the alerts name as
            // `/nproj/<id>`, or its .com slug): the same page, two keys.
            let mut stmt = conn.prepare_cached(
                "SELECT portal, job_id, title, company FROM job
                 WHERE (portal <> ?1
                        OR (job_id <> ?2 AND (?3 OR job_id GLOB 'u*')))
                   AND desc_status IN ('ok', 'teaser') AND dup_of IS NULL
                   AND archived_at IS NULL AND trashed_at IS NULL AND desc_closed = 0
                 ORDER BY first_seen_at, portal, job_id",
            )?;
            let own_hash = !key.has_portal_id();
            let candidates: Vec<(String, String)> = stmt
                .query_map(params![key.portal.key(), key.id, own_hash], |r| {
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
            for (portal, id) in candidates {
                let Some(other) = row(conn, &portal, &id, false)? else {
                    continue;
                };
                if !same_ad(&own, &other) {
                    continue;
                }
                let Some(portal) = Portal::from_key(&portal) else {
                    continue;
                };
                let other_key = JobKey { portal, id };
                // The full text arrived after its teaser, or the job with the portal's id
                // after the same page under a slug key: it becomes the original, and the
                // other row (with whatever pointed to it) points to it - unless the user
                // marked that row, or this ad is closed.
                let same_portal = other_key.portal == key.portal;
                let id_after_slug =
                    same_portal && key.has_portal_id() && !other_key.has_portal_id();
                if (other.teaser && !own.teaser) || id_after_slug {
                    if other.marked || own.closed {
                        continue;
                    }
                    link(conn, &other_key, key)?;
                    conn.execute(
                        "UPDATE job SET dup_of = ?1 WHERE dup_of = ?2",
                        params![key.to_string(), other_key.to_string()],
                    )?;
                    inherit(conn, key, &other_key)?;
                    return Ok(Some(key.clone()));
                }
                if own.marked {
                    return Ok(None);
                }
                link(conn, key, &other_key)?;
                return Ok(Some(other_key));
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
            // The same portal under a second key (a slug link) is no other portal.
            if keys.contains(&&original) && portal != original.portal {
                let portals = out.entry(original).or_default();
                if !portals.contains(&portal) {
                    portals.push(portal);
                }
            }
        }
        Ok(out)
    }
}

/// What the comparison needs of a row.
struct Row {
    title: String,
    company: String,
    text: String,
    /// Only a guest's teaser, not the full text.
    teaser: bool,
    closed: bool,
    /// The user marked it (a favourite, moved out of the inbox, "fits anyway").
    marked: bool,
}

/// A row with a text; `open`: only one that may still be linked (a full text or teaser,
/// not yet a duplicate).
fn row(conn: &rusqlite::Connection, portal: &str, id: &str, open: bool) -> Result<Option<Row>> {
    Ok(conn
        .query_row(
            "SELECT title, company, desc_text, desc_status = 'teaser', desc_closed,
                    app_status IS NOT NULL OR archived_at IS NOT NULL
                      OR trashed_at IS NOT NULL OR override_include IS NOT NULL
             FROM job
             WHERE portal = ?1 AND job_id = ?2 AND desc_text IS NOT NULL
               AND (NOT ?3 OR (desc_status IN ('ok', 'teaser') AND dup_of IS NULL))",
            params![portal, id, open],
            |r| {
                Ok(Row {
                    title: r.get(0)?,
                    company: r.get(1)?,
                    text: r.get(2)?,
                    teaser: r.get(3)?,
                    closed: r.get(4)?,
                    marked: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// Share of a teaser's word triples (in percent) that the full text must contain.
pub const MIN_CONTAINED: usize = 80;
/// Fewest word triples a teaser needs to be compared at all (a few words prove nothing).
const MIN_TRIPLES: usize = 8;

/// Do the two texts announce the same ad? Two full texts (or two teasers) by `SimHash`, a
/// teaser and a full text by containment.
fn same_ad(a: &Row, b: &Row) -> bool {
    match (a.teaser, b.teaser) {
        (true, false) => contained(&a.text, &b.text),
        (false, true) => contained(&b.text, &a.text),
        _ => distance(simhash(&a.text), simhash(&b.text)) <= MAX_DISTANCE,
    }
}

/// Are at least [`MIN_CONTAINED`] percent of the teaser's word triples in the full text?
pub(crate) fn contained(teaser: &str, full: &str) -> bool {
    let part = triples(teaser);
    if part.len() < MIN_TRIPLES {
        return false;
    }
    let whole = triples(full);
    let hits = part.iter().filter(|t| whole.contains(*t)).count();
    hits * 100 >= part.len() * MIN_CONTAINED
}

/// The word triples of a text (lower case, letters and digits only).
fn triples(text: &str) -> std::collections::HashSet<String> {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words.windows(3).map(|w| w.join(" ")).collect()
}

/// `key` goes into `original`'s row: its own score is gone for good (never scored again,
/// never listed on its own).
fn link(conn: &rusqlite::Connection, key: &JobKey, original: &JobKey) -> Result<()> {
    conn.execute(
        "UPDATE job SET dup_of = ?3, match_score = NULL, match_status = NULL,
                        match_note = NULL, match_at = NULL, match_rev = NULL
         WHERE portal = ?1 AND job_id = ?2",
        params![key.portal.key(), key.id, original.to_string()],
    )?;
    bump(conn)
}

/// What a row that becomes the original takes over from the row it replaces (a teaser or
/// a slug row the user may have read): read stays read (the earlier time), the job counts
/// from its first sighting - so "Neu", the run card's new jobs and the age that archives it
/// treat it as the job the user already saw - and a text file the other row got stays the
/// job's one file (the new original gets none of its own).
fn inherit(conn: &rusqlite::Connection, original: &JobKey, replaced: &JobKey) -> Result<()> {
    struct Kept {
        read_at: Option<i64>,
        first_seen_at: i64,
        first_seen_run: i64,
        txt_name: Option<String>,
        txt_written_at: Option<i64>,
    }
    let kept = |key: &JobKey| {
        conn.query_row(
            "SELECT read_at, first_seen_at, first_seen_run, txt_name, txt_written_at
             FROM job WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id],
            |r| {
                Ok(Kept {
                    read_at: r.get(0)?,
                    first_seen_at: r.get(1)?,
                    first_seen_run: r.get(2)?,
                    txt_name: r.get(3)?,
                    txt_written_at: r.get(4)?,
                })
            },
        )
    };
    let (new, old) = (kept(original)?, kept(replaced)?);
    let read_at = match (new.read_at, old.read_at) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let hand_over = new.txt_name.is_none() && old.txt_name.is_some();
    let (txt_name, txt_written_at) = if hand_over {
        (old.txt_name, old.txt_written_at)
    } else {
        (new.txt_name, new.txt_written_at)
    };
    conn.execute(
        "UPDATE job SET read_at = ?3, first_seen_at = ?4, first_seen_run = ?5,
                        txt_name = ?6, txt_written_at = ?7
         WHERE portal = ?1 AND job_id = ?2",
        params![
            original.portal.key(),
            original.id,
            read_at,
            new.first_seen_at.min(old.first_seen_at),
            new.first_seen_run.min(old.first_seen_run),
            txt_name,
            txt_written_at,
        ],
    )?;
    if hand_over {
        conn.execute(
            "UPDATE job SET txt_name = NULL, txt_written_at = NULL
             WHERE portal = ?1 AND job_id = ?2",
            params![replaced.portal.key(), replaced.id],
        )?;
    }
    Ok(())
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
            rank: 0,
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
        let overview = store.overview_jobs(20).unwrap();
        assert!(overview.favourites.is_empty());
        assert_eq!(keys(overview.new), [first.key]);
        assert_eq!(overview.new_total, 1);
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

    /// The start of the ad, as freelance.de shows it to a guest.
    fn teaser_of(text: &str) -> String {
        text.chars().take(260).collect()
    }

    #[test]
    fn a_teaser_is_contained_in_its_full_text_only() {
        assert!(contained(&teaser_of(TEXT), TEXT));
        let other = "Wir suchen eine Projektleitung für den Rollout eines Warenwirtschaftssystems \
            in 40 Filialen. Aufgaben: Planung, Steuerung der Dienstleister, Berichtswesen an \
            die Geschäftsführung. Profil: Erfahrung im Handel und in agilen Methoden.";
        assert!(!contained(&teaser_of(TEXT), other));
        assert!(!contained("Für unseren Kunden", TEXT), "too little to tell");
        // A teaser is never within a few SimHash bits of the whole ad.
        assert!(distance(simhash(&teaser_of(TEXT)), simhash(TEXT)) > MAX_DISTANCE);
    }

    /// freelance.de as a guest gives teasers: the same project on another portal with its
    /// full text is one row, and the listed row is the one with the full text - whichever
    /// arrived first.
    #[test]
    fn a_teaser_points_to_the_full_text_whichever_came_first() {
        use crate::store::test_support::{mail, now, posting};
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let add = |url: &str| {
            let p = posting(
                url,
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            p.key
        };
        // The full text first, the teaser later.
        let full = add("https://www.freelancermap.de/nproj/12345.html");
        store.record_text(&full, TEXT, false, false, now()).unwrap();
        let teaser = add("https://www.freelance.de/project/index.php?id=1255067");
        store
            .record_teaser(&teaser, &teaser_of(TEXT), now())
            .unwrap();
        assert_eq!(store.link_duplicate(&teaser).unwrap(), Some(full.clone()));
        assert_eq!(store.dup_of(&teaser).unwrap(), Some(full));
    }

    #[test]
    fn a_full_text_after_its_teaser_becomes_the_original() {
        use crate::store::test_support::{mail, now, posting};
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let add = |url: &str| {
            let p = posting(
                url,
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            p.key
        };
        let teaser = add("https://www.freelance.de/project/index.php?id=1255068");
        store
            .record_teaser(&teaser, &teaser_of(TEXT), now())
            .unwrap();
        assert_eq!(store.link_duplicate(&teaser).unwrap(), None);
        let full = add("https://www.linkedin.com/jobs/view/4000000009/");
        store.record_text(&full, TEXT, false, false, now()).unwrap();
        assert_eq!(store.link_duplicate(&full).unwrap(), Some(full.clone()));
        assert_eq!(store.dup_of(&teaser).unwrap(), Some(full.clone()));
        assert_eq!(
            store.dup_of(&full).unwrap(),
            None,
            "the full text is listed"
        );
    }

    /// The full text that takes a read teaser's place is the job the user already saw: it
    /// stays read, keeps its first sighting and is no new job of the run that brought it.
    #[test]
    fn the_new_original_keeps_what_the_user_saw() {
        use crate::store::test_support::{mail, now, posting};
        let store = Store::in_memory().unwrap();
        let earlier = now() - jiff::SignedDuration::from_hours(48);
        let add = |url: &str, run: i64, at| {
            let p = posting(
                url,
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &p, mail(), at).unwrap();
            p.key
        };
        let first_run = store.begin_run().unwrap();
        let teaser = add(
            "https://www.freelance.de/project/index.php?id=1255068",
            first_run,
            earlier,
        );
        store
            .record_teaser(&teaser, &teaser_of(TEXT), earlier)
            .unwrap();
        assert!(store.mark_read(&teaser, earlier).unwrap());
        let run = store.begin_run().unwrap();
        let full = add("https://www.linkedin.com/jobs/view/4000000009/", run, now());
        store.record_text(&full, TEXT, false, false, now()).unwrap();
        assert_eq!(store.link_duplicate(&full).unwrap(), Some(full.clone()));
        let job = store.job(&full).unwrap().unwrap();
        assert_eq!(job.read_at, Some(earlier), "read stays read");
        assert_eq!(
            (job.first_seen_at, job.first_seen_run),
            (earlier, first_run),
            "its first sighting"
        );
        assert_eq!(
            store.new_jobs(run).unwrap(),
            (0, 0),
            "no new job of the run"
        );
    }

    /// freelancermap's `/projekt/<slug>` link carries no id, its alerts link `/nproj/<id>`:
    /// the same project under two keys is one row - the one with the portal's id - in
    /// either order; two different projects of the portal never merge.
    #[test]
    fn a_slug_link_and_an_id_link_of_one_project_are_one_row() {
        use crate::store::test_support::{mail, now, posting};
        for id_first in [true, false] {
            let store = Store::in_memory().unwrap();
            let run = store.begin_run().unwrap();
            let add = |url: &str| {
                let p = posting(
                    url,
                    "SAP FI/CO Berater (m/w/d)",
                    "Ferrum Systems SE",
                    "Hamburg",
                );
                store.upsert_posting(run, &p, mail(), now()).unwrap();
                store
                    .record_text(&p.key, TEXT, false, false, now())
                    .unwrap();
                p.key
            };
            let (id, slug) = if id_first {
                let id = add("https://www.freelancermap.de/nproj/2971857.html");
                (
                    id,
                    add("https://www.freelancermap.de/projekt/sap-fi-co-berater-m-w-d"),
                )
            } else {
                let slug = add("https://www.freelancermap.de/projekt/sap-fi-co-berater-m-w-d");
                (add("https://www.freelancermap.de/nproj/2971857.html"), slug)
            };
            assert!(!slug.has_portal_id());
            // The slug row got its text file in an earlier run.
            let name = "20260918_Freelancermap_SAP_FI_CO_Berater.txt";
            if !id_first {
                store.mark_txt_written(&slug, name, now()).unwrap();
            }
            let later = if id_first { &slug } else { &id };
            assert_eq!(store.link_duplicate(later).unwrap(), Some(id.clone()));
            assert_eq!(store.dup_of(&slug).unwrap(), Some(id.clone()), "{id_first}");
            assert_eq!(store.dup_of(&id).unwrap(), None);
            assert!(store.also_on(&[&id]).unwrap().is_empty(), "no other portal");
            if !id_first {
                // That file stays the job's one file: the id row takes it over.
                assert_eq!(
                    store.job(&id).unwrap().unwrap().txt_name.as_deref(),
                    Some(name)
                );
                assert_eq!(store.job(&slug).unwrap().unwrap().txt_name, None);
                assert!(store.txt_jobs(false).unwrap().is_empty(), "no second file");
                assert_eq!(store.txt_names().unwrap(), [name]);
            }
        }
        // Two projects of the portal, both with an id: never merged, however alike.
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        for url in [
            "https://www.freelancermap.de/nproj/2971857.html",
            "https://www.freelancermap.de/nproj/2971858.html",
        ] {
            let p = posting(
                url,
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            store
                .record_text(&p.key, TEXT, false, false, now())
                .unwrap();
            assert_eq!(store.link_duplicate(&p.key).unwrap(), None);
        }
    }

    /// An archived or closed original never swallows a fresh announcement of the same job.
    #[test]
    fn an_archived_or_closed_job_is_no_original() {
        use crate::store::test_support::{mail, now, posting};
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let add = |url: &str| {
            let p = posting(
                url,
                "SAP FI/CO Berater (m/w/d)",
                "Ferrum Systems SE",
                "Hamburg",
            );
            store.upsert_posting(run, &p, mail(), now()).unwrap();
            p.key
        };
        let archived = add("https://www.freelancermap.de/nproj/12345.html");
        store
            .record_text(&archived, TEXT, false, false, now())
            .unwrap();
        store
            .move_jobs(
                std::slice::from_ref(&archived),
                crate::model::Place::Archive,
                now(),
            )
            .unwrap();
        let closed = add("https://www.freelance.de/project/index.php?id=1255067");
        store
            .record_text(&closed, TEXT, false, true, now())
            .unwrap();
        let fresh = add("https://www.linkedin.com/jobs/view/4000000001/");
        store
            .record_text(&fresh, TEXT, false, false, now())
            .unwrap();
        assert_eq!(store.link_duplicate(&fresh).unwrap(), None);
        assert_eq!(store.dup_of(&fresh).unwrap(), None);
    }
}
