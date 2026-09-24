//! Placeholder for schema 3: match results and the per-job state around them.
//!
//! Nothing here touches the database yet. A later track adds the migration from schema 2 to
//! 3 (in `schema`) and the methods that read and write these columns; until then this module
//! only pins down their names and types so that the parallel tracks agree on them. Every
//! column is nullable, so the migration is one `ALTER TABLE job ADD COLUMN` per entry and
//! existing rows need no backfill.

/// The nullable columns schema 3 adds to the `job` table, as `(name, sql_type)` pairs.
/// Timestamps are Unix seconds, like every other time column.
///
/// - `match_score`: score 0-100.
/// - `match_status`: `scored`, `excluded` or `unscorable`.
/// - `match_note`: JSON `{code, params, mustMet, mustTotal, top[<=2]}`, at most 400 bytes.
/// - `match_at`: when the job was scored.
/// - `match_rev`: 16 hex digits of a SHA-256 over engine version, canonical profile view and
///   model id - tells which engine and profile produced the score.
/// - `read_at`: when the user read the job; `NULL` means unread.
/// - `desc_facts`: facts taken from the job page.
/// - `dup_of`: `portal:id` of the job this one duplicates.
/// - `parser_version`: version of the page parser that produced the full text.
/// - `pinned_at`: when the user pinned the job; `NULL` means not pinned.
pub const SCHEMA_3_JOB_COLUMNS: &[(&str, &str)] = &[
    ("match_score", "INTEGER"),
    ("match_status", "TEXT"),
    ("match_note", "TEXT"),
    ("match_at", "INTEGER"),
    ("match_rev", "TEXT"),
    ("read_at", "INTEGER"),
    ("desc_facts", "TEXT"),
    ("dup_of", "TEXT"),
    ("parser_version", "INTEGER"),
    ("pinned_at", "INTEGER"),
];

#[cfg(test)]
mod tests {
    use super::SCHEMA_3_JOB_COLUMNS;
    use crate::store::Store;

    /// The planned columns must really be new: a name the schema 2 `job` table already has
    /// would make the migration fail with "duplicate column name".
    #[test]
    fn schema_3_columns_are_not_in_schema_2() {
        let store = Store::in_memory().unwrap();
        let conn = store.conn();
        let mut stmt = conn.prepare("PRAGMA table_info(job)").unwrap();
        let existing: Vec<String> = stmt
            .query_map([], |r| r.get(1))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(existing.contains(&"search".to_string()), "no job table");
        for (name, _) in SCHEMA_3_JOB_COLUMNS {
            assert!(
                !existing.iter().any(|c| c == name),
                "`{name}` already exists in schema 2"
            );
        }
    }
}
