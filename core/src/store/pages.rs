//! What the page parsers leave behind besides the text: the facts a page states
//! (`desc_facts`) and the parser version that judged it (`parser_version`).

use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::Result;
use crate::portal::{Facts, JobKey};

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
        self.conn().execute(
            "UPDATE job SET parser_version = ?3,
                            desc_facts = CASE WHEN ?4 THEN ?5 ELSE desc_facts END
             WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id, parser_version, replace, json],
        )?;
        Ok(())
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
