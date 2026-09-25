//! What the page parsers leave behind besides the text: the facts a page states
//! (`desc_facts`) and the parser version that judged it (`parser_version`).

use jiff::Timestamp;
use rusqlite::{OptionalExtension, params};

use super::jobs::FETCHABLE;
use super::{Store, bump};
use crate::error::Result;
use crate::portal::{Facts, JobKey, Portal};
use crate::time::to_db;

impl Store {
    /// The parser that judged the job last, and the facts its page stated (`None` keeps the
    /// stored facts - a failed page states nothing).
    pub fn record_parse(
        &self,
        key: &JobKey,
        parser_version: u32,
        facts: Option<&Facts>,
    ) -> Result<()> {
        // A new page replaces the facts (none stated = none stored).
        let replace = facts.is_some();
        let json = facts
            .filter(|f| !f.is_empty())
            .map(|f| serde_json::to_string(f).expect("facts are always serialisable"));
        // Other facts judge the job anew (the engine reads them); SET sees the old values.
        self.conn().execute(
            "UPDATE job SET parser_version = ?3,
                            desc_facts = CASE WHEN ?4 THEN ?5 ELSE desc_facts END,
                            match_rev = CASE WHEN ?4 AND desc_facts IS NOT ?5 THEN NULL
                                             ELSE match_rev END
             WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id, parser_version, replace, json],
        )?;
        Ok(())
    }

    /// After a parser update: the jobs of `portal` that an older parser judged as failed or
    /// unfetchable are open again, with fresh attempts - only those the automatic queue
    /// fetches (mail since `since`, a job the lists show as active), so the count is truthful
    /// and any other job keeps its honest "not fetchable" instead of waiting for a fetch that
    /// never comes. Returns their number.
    pub fn requeue_older_parses(
        &self,
        portal: Portal,
        parser_version: u32,
        since: Timestamp,
    ) -> Result<usize> {
        self.write(|conn| {
            let changed = conn.execute(
                &format!(
                    "UPDATE job SET desc_status = 'missing', desc_attempts = 0, desc_error = NULL,
                                    desc_attempted_at = NULL
                     WHERE portal = ?1 AND desc_status IN ('failed', 'unfetchable')
                       AND COALESCE(parser_version, 0) < ?2
                       AND COALESCE(mail_date, first_seen_at) >= ?3 AND {FETCHABLE}"
                ),
                params![portal.key(), parser_version, to_db(since)],
            )?;
            if changed > 0 {
                bump(conn)?;
            }
            Ok(changed)
        })
    }

    /// The parser version that judged the job last (`None`: never judged, or before
    /// versions existed).
    pub fn parser_version(&self, key: &JobKey) -> Result<Option<u32>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT parser_version FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// The facts the job page stated (unreadable JSON counts as none).
    pub fn facts(&self, key: &JobKey) -> Result<Option<Facts>> {
        let json: Option<String> = self
            .conn()
            .query_row(
                "SELECT desc_facts FROM job WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }
}
