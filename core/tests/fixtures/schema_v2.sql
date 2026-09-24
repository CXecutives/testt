-- Frozen schema 2: the database as it looked before matching (user_version 2).
-- Never change this file together with the code - otherwise the migration tests would
-- check their own present instead of the way there.
CREATE TABLE job (
    portal            TEXT    NOT NULL,
    job_id            TEXT    NOT NULL,
    url               TEXT    NOT NULL,
    title             TEXT    NOT NULL,
    company           TEXT    NOT NULL,
    location          TEXT    NOT NULL,
    mail_date         INTEGER,
    mail_subject      TEXT    NOT NULL,
    gmail_id          TEXT,
    first_seen_at     INTEGER NOT NULL,
    first_seen_run    INTEGER NOT NULL,
    last_seen_run     INTEGER NOT NULL,
    desc_status       TEXT    NOT NULL DEFAULT 'missing',
    desc_short        INTEGER NOT NULL DEFAULT 0,
    desc_closed       INTEGER NOT NULL DEFAULT 0,
    desc_text         TEXT,
    desc_fetched_at   INTEGER,
    desc_attempted_at INTEGER,
    desc_attempts     INTEGER NOT NULL DEFAULT 0,
    desc_error        TEXT,
    txt_name          TEXT,
    txt_written_at    INTEGER,
    search            TEXT    NOT NULL,
    PRIMARY KEY (portal, job_id)
) WITHOUT ROWID;
CREATE INDEX job_by_run ON job (first_seen_run);
CREATE TABLE alert_mail (
    mail_key       TEXT    PRIMARY KEY,
    portal         TEXT    NOT NULL,
    subject        TEXT    NOT NULL,
    mail_date      INTEGER,
    gmail_id       TEXT,
    n_postings     INTEGER NOT NULL,
    last_seen_run  INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE kv (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
