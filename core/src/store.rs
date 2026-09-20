//! Das Gedächtnis der App: eine SQLite-Datei, die einzige Wahrheit über Jobs, Mails und
//! Jobdetails. Excel, CSV und TXT werden daraus erzeugt – nie umgekehrt.
//!
//! Eine Verbindung hinter einem Mutex; jede Methode ist kurz und synchron (der Aufrufer
//! hält die Sperre nie über ein `await`).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use jiff::{SignedDuration, Timestamp};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use url::Url;

use crate::error::{Error, Result};
use crate::model::{
    AlertMail, DescStatus, MAX_FIELD_CHARS, MAX_TITLE_CHARS, Posting, TITLE_PLACEHOLDER,
};
use crate::portal::{JobKey, Portal};
use crate::text::{one_line, page_location, split_company_location, truncate_chars};
use crate::time::{from_db, to_db};

/// Nach so vielen erfolglosen Versuchen gilt ein Job als nicht abrufbar.
const MAX_FETCH_ATTEMPTS: i64 = 3;
/// Höchstlänge eines gespeicherten Fehlergrunds (in Zeichen).
const MAX_ERROR_CHARS: usize = 200;

const SCHEMA_VERSION: i64 = 2;

/// Von Schema 1 auf 2: `alert_mail` hat zwei nie benutzte Spalten verloren. Die Tabelle ist
/// nur ein Gedächtnis für „Mail ohne erkannte Einträge“ und baut sich beim nächsten Abruf
/// neu auf – deshalb genügt Neuanlegen statt Umkopieren.
const MIGRATE_1_TO_2: &str = "
DROP TABLE alert_mail;
CREATE TABLE alert_mail (
    mail_key       TEXT    PRIMARY KEY,
    portal         TEXT    NOT NULL,
    subject        TEXT    NOT NULL,
    mail_date      INTEGER,
    gmail_id       TEXT,
    n_postings     INTEGER NOT NULL,
    last_seen_run  INTEGER NOT NULL
) WITHOUT ROWID;
";

const SCHEMA: &str = "
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
";

/// Wie ein Eintrag im laufenden Scan einzuordnen ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Zum ersten Mal gesehen.
    New,
    /// Schon aus einem früheren Lauf bekannt.
    KnownBefore,
    /// In diesem Lauf schon einmal vorgekommen (derselbe Job in zwei Alert-Mails).
    DupInRun,
}

/// Ein Job, wie ihn Oberfläche und Export sehen (ohne Volltext).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRow {
    pub key: JobKey,
    pub url: Url,
    pub title: String,
    pub company: String,
    pub location: String,
    pub mail_date: Option<Timestamp>,
    pub mail_subject: String,
    pub gmail_id: Option<u64>,
    pub first_seen_at: Timestamp,
    pub first_seen_run: i64,
    pub desc_status: DescStatus,
    pub desc_short: bool,
    pub desc_closed: bool,
    pub desc_len: i64,
    pub desc_fetched_at: Option<Timestamp>,
    pub desc_attempts: i64,
    pub desc_error: Option<String>,
    pub txt_name: Option<String>,
}

/// Jobs eines Portals mit einem bestimmten Jobdetails-Stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortalCount {
    pub portal: Portal,
    pub status: DescStatus,
    pub count: i64,
    /// Davon mit kurzem Text.
    pub short: i64,
}

/// Eine Alert-Mail ohne erkannte Einträge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertMailRow {
    pub portal: Portal,
    pub subject: String,
    pub mail_date: Option<Timestamp>,
    pub gmail_id: Option<u64>,
}

/// Auswahl für Liste und Export.
#[derive(Debug, Clone, Default)]
pub struct JobFilter {
    /// Nur Jobs, die in diesem Lauf zum ersten Mal gesehen wurden.
    pub first_seen_run: Option<i64>,
    /// Suchbegriff in Titel, Firma, Ort und Volltext (Groß/klein egal).
    pub search: Option<String>,
}

/// Mail-Angaben, die ein Job bei seiner ersten Sichtung übernimmt.
#[derive(Debug, Clone, Copy)]
pub struct MailRef<'a> {
    pub subject: &'a str,
    pub date: Option<Timestamp>,
    pub gmail_id: Option<u64>,
}

impl<'a> From<&'a AlertMail> for MailRef<'a> {
    fn from(mail: &'a AlertMail) -> Self {
        MailRef {
            subject: &mail.subject,
            date: mail.date,
            gmail_id: mail.gmail_id,
        }
    }
}

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Öffnet (oder erzeugt) die Datenbank und bringt das Schema auf den Stand.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Datenbank nur im Arbeitsspeicher (Trockenlauf, Tests).
    pub fn in_memory() -> Result<Store> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            // Schema und Versionsnummer in einer Transaktion: Ein Absturz dazwischen
            // hinterließe sonst Tabellen mit Version 0, und jeder weitere Start scheiterte.
            0 => conn.execute_batch(&format!(
                "BEGIN IMMEDIATE; {SCHEMA} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
            ))?,
            // Ältere Datenbank: Schritt für Schritt hochziehen, jeder Schritt in einer Transaktion.
            1 => conn.execute_batch(&format!(
                "BEGIN IMMEDIATE; {MIGRATE_1_TO_2} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
            ))?,
            SCHEMA_VERSION => {}
            newer => return Err(Error::NewerSchema(newer)),
        }
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        // Eine Panik in einem anderen Aufrufer lässt die Verbindung intakt (SQLite rollt
        // offene Transaktionen selbst zurück) – deshalb die Vergiftung ignorieren.
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Mehrere Anweisungen als **eine** Änderung: ganz oder gar nicht – ein Absturz
    /// dazwischen hinterließe sonst etwa einen Volltext ohne Suchspalte oder ohne neuen
    /// Änderungszähler – und ein Schreibvorgang auf die Platte statt mehrerer.
    fn write<T>(&self, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn();
        let tx = Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
        let value = work(&tx)?;
        tx.commit()?;
        Ok(value)
    }

    // ------------------------------------------------------------------ Läufe

    /// Beginnt einen Lauf und liefert seine Nummer (fortlaufend, über Neustarts hinweg).
    pub fn begin_run(&self) -> Result<i64> {
        let conn = self.conn();
        let next = kv_get_i64(&conn, "run_seq")?.unwrap_or(0) + 1;
        kv_set(&conn, "run_seq", &next.to_string())?;
        Ok(next)
    }

    /// Beginn des letzten vollständig erfolgreichen Postfach-Scans für ein Portal.
    pub fn last_scan(&self, portal: Portal) -> Result<Option<Timestamp>> {
        Ok(kv_get_i64(&self.conn(), &scan_key(portal))?.and_then(from_db))
    }

    /// Vergisst den Scan-Stand aller Portale – nach einem Wechsel des Gmail-Kontos beginnt
    /// das neue Postfach mit dem Erstlauf (7 Tage) statt beim Stand des alten.
    pub fn clear_scan_state(&self) -> Result<()> {
        self.conn()
            .execute("DELETE FROM kv WHERE key LIKE 'last_scan:%'", [])?;
        Ok(())
    }

    pub fn set_last_scan(&self, portal: Portal, at: Timestamp) -> Result<()> {
        kv_set(&self.conn(), &scan_key(portal), &to_db(at).to_string())
    }

    // ------------------------------------------------------------------ Aufnahme

    /// Nimmt einen Eintrag aus einer Alert-Mail auf (Merge-Regel: [`upsert`]).
    pub fn upsert_posting(
        &self,
        run: i64,
        posting: &Posting,
        mail: MailRef<'_>,
        now: Timestamp,
    ) -> Result<Seen> {
        self.write(|conn| upsert(conn, run, posting, mail, now))
    }

    /// Nimmt eine erkannte Alert-Mail samt ihren Einträgen auf – als eine Änderung. Auch eine
    /// Mail ohne Einträge wird gemerkt (Layout-Wächter). Liefert je Eintrag die Einordnung.
    pub fn record_alert(&self, run: i64, alert: &AlertMail, now: Timestamp) -> Result<Vec<Seen>> {
        self.write(|conn| {
            conn.execute(
                "INSERT INTO alert_mail (mail_key, portal, subject, mail_date, gmail_id,
                                         n_postings, last_seen_run)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT (mail_key) DO UPDATE SET
                    n_postings = excluded.n_postings, last_seen_run = excluded.last_seen_run",
                params![
                    alert.key,
                    alert.portal.key(),
                    alert.subject,
                    alert.date.map(to_db),
                    alert.gmail_id.map(|id| id.to_string()),
                    i64::try_from(alert.postings.len()).unwrap_or(i64::MAX),
                    run,
                ],
            )?;
            alert
                .postings
                .iter()
                .map(|posting| upsert(conn, run, posting, MailRef::from(alert), now))
                .collect()
        })
    }

    /// Alert-Mails ohne einen erkannten Eintrag aus einem Lauf (Layout-Wächter: graue
    /// Zeilen mit Gmail-Link).
    pub fn zero_posting_mails(&self, run: i64) -> Result<Vec<AlertMailRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, subject, mail_date, gmail_id FROM alert_mail
             WHERE last_seen_run = ?1 AND n_postings = 0 ORDER BY mail_date DESC",
        )?;
        let rows = stmt.query_map([run], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, subject, date, gmail_id) = row?;
            let portal = Portal::from_key(&portal)
                .ok_or_else(|| Error::Corrupt(format!("Portal „{portal}“")))?;
            out.push(AlertMailRow {
                portal,
                subject,
                mail_date: date.and_then(from_db),
                gmail_id: gmail_id.and_then(|id| id.parse().ok()),
            });
        }
        Ok(out)
    }

    // ------------------------------------------------------------------ Abfragen

    pub fn job(&self, key: &JobKey) -> Result<Option<JobRow>> {
        let conn = self.conn();
        let row = conn
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM job WHERE portal = ?1 AND job_id = ?2"),
                params![key.portal.key(), key.id],
                job_row,
            )
            .optional()?;
        row.transpose()
    }

    /// Volltext eines Jobs (nur mit Status „ok“).
    pub fn description(&self, key: &JobKey) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT desc_text FROM job WHERE portal = ?1 AND job_id = ?2 AND desc_status = 'ok'",
                params![key.portal.key(), key.id],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// Jobs, neueste zuerst (Erstsichtung, dann Mail-Datum).
    pub fn jobs(&self, filter: &JobFilter) -> Result<Vec<JobRow>> {
        let conn = self.conn();
        let pattern = filter
            .search
            .as_deref()
            .map(|s| format!("%{}%", escape_like(&fold(s.trim()))))
            .filter(|p| p != "%%");
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job
             WHERE (?1 IS NULL OR first_seen_run = ?1)
               AND (?2 IS NULL OR search LIKE ?2 ESCAPE '\\')
             ORDER BY first_seen_at DESC, mail_date DESC, portal, job_id"
        ))?;
        let rows = stmt.query_map(params![filter.first_seen_run, pattern], job_row)?;
        rows.map(|r| r?).collect()
    }

    /// Anzahl aller Jobs.
    pub fn job_count(&self) -> Result<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM job", [], |r| r.get(0))?)
    }

    /// Je Portal und Jobdetails-Stand: Anzahl und davon „kurz“ (für die Portal-Ansicht).
    pub fn portal_counts(&self) -> Result<Vec<PortalCount>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT portal, desc_status, COUNT(*), SUM(desc_short) FROM job GROUP BY portal, desc_status",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, status, count, short) = row?;
            out.push(PortalCount {
                portal: Portal::from_key(&portal)
                    .ok_or_else(|| Error::Corrupt(format!("Portal „{portal}“")))?,
                status: DescStatus::parse(&status)
                    .ok_or_else(|| Error::Corrupt(format!("Status „{status}“")))?,
                count,
                short,
            });
        }
        Ok(out)
    }

    // ------------------------------------------------------------------ Jobdetails

    /// Jobs, deren Volltext automatisch geholt werden soll: offen oder fehlgeschlagen
    /// (frühestens `retry_after` nach dem letzten Versuch), Mail höchstens `max_age` alt.
    /// Reihenfolge: offene vor Wiederholungen, dann neueste Mail zuerst.
    pub fn fetch_queue(
        &self,
        now: Timestamp,
        max_age: SignedDuration,
        retry_after: SignedDuration,
    ) -> Result<Vec<JobRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS} FROM job WHERE {DUE}
             ORDER BY desc_status = 'failed', COALESCE(mail_date, first_seen_at) DESC, portal, job_id"
        ))?;
        let rows = stmt.query_map(due_params(now, max_age, retry_after), job_row)?;
        rows.map(|r| r?).collect()
    }

    /// Je Portal die Zahl der Jobs in der Warteschlange (wie [`Store::fetch_queue`]).
    pub fn due_counts(
        &self,
        now: Timestamp,
        max_age: SignedDuration,
        retry_after: SignedDuration,
    ) -> Result<Vec<(Portal, usize)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT portal, COUNT(*) FROM job WHERE {DUE} GROUP BY portal"
        ))?;
        let rows = stmt.query_map(due_params(now, max_age, retry_after), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (portal, count) = row?;
            let portal = Portal::from_key(&portal)
                .ok_or_else(|| Error::Corrupt(format!("Portal „{portal}“")))?;
            out.push((portal, usize::try_from(count).unwrap_or(0)));
        }
        Ok(out)
    }

    /// Volltext gespeichert (auch kurze, geprüfte Texte und geschlossene Anzeigen).
    pub fn record_text(
        &self,
        key: &JobKey,
        text: &str,
        short: bool,
        closed: bool,
        now: Timestamp,
    ) -> Result<()> {
        self.write(|conn| {
            conn.execute(
                "UPDATE job SET desc_status = 'ok', desc_text = ?3, desc_short = ?4, desc_closed = ?5,
                                desc_fetched_at = ?6, desc_attempted_at = ?6, desc_error = NULL
                 WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id, text, short, closed, to_db(now)],
            )?;
            bump(conn)?;
            refresh_search(conn, key)
        })
    }

    /// Anzeige gibt es nicht mehr.
    pub fn record_gone(&self, key: &JobKey, now: Timestamp) -> Result<()> {
        self.write(|conn| {
            // Ein schon geholter Text bleibt gültig – Erfolgreiches wird nie zurückgestuft.
            let changed = conn.execute(
                "UPDATE job SET desc_status = 'gone', desc_attempted_at = ?3, desc_error = NULL
                 WHERE portal = ?1 AND job_id = ?2 AND desc_status <> 'ok'",
                params![key.portal.key(), key.id, to_db(now)],
            )?;
            if changed > 0 {
                bump(conn)?;
            }
            Ok(())
        })
    }

    /// Seite geladen, aber kein gültiger Text: Versuch zählen; nach
    /// `MAX_FETCH_ATTEMPTS` gilt der Job als nicht abrufbar. Ein Job mit gültigem Text
    /// bleibt unverändert (Rückgabe: sein Status). Der Grund stammt oft aus der Seite
    /// (Umleitungsziel, Skriptfehler) – er wird einzeilig und kurz gespeichert.
    pub fn record_failed(&self, key: &JobKey, error: &str, now: Timestamp) -> Result<DescStatus> {
        let error = truncate_chars(&one_line(error), MAX_ERROR_CHARS);
        let status: String = self.write(|conn| {
            let updated: Option<String> = conn
                .query_row(
                    "UPDATE job SET desc_attempts = desc_attempts + 1, desc_attempted_at = ?3, desc_error = ?4,
                                    desc_status = CASE WHEN desc_attempts + 1 >= ?5 THEN 'unfetchable' ELSE 'failed' END
                     WHERE portal = ?1 AND job_id = ?2 AND desc_status <> 'ok'
                     RETURNING desc_status",
                    params![key.portal.key(), key.id, to_db(now), error, MAX_FETCH_ATTEMPTS],
                    |r| r.get(0),
                )
                .optional()?;
            match updated {
                Some(status) => {
                    bump(conn)?;
                    Ok(status)
                }
                None => Ok(conn.query_row(
                    "SELECT desc_status FROM job WHERE portal = ?1 AND job_id = ?2",
                    params![key.portal.key(), key.id],
                    |r| r.get(0),
                )?),
            }
        })?;
        DescStatus::parse(&status).ok_or_else(|| Error::Corrupt(format!("Status „{status}“")))
    }

    /// Strukturierte Seitenangaben (LinkedIn-Kopf, freelancermap-Daten) sind verlässlicher
    /// als die Mail-Heuristik: nicht leere Werte ersetzen die Mail-Werte – mit denselben
    /// Längengrenzen wie bei der Aufnahme, und die Arbeitsform der Mail („Remote“) bleibt.
    pub fn record_page_fields(
        &self,
        key: &JobKey,
        title: &str,
        company: &str,
        location: &str,
    ) -> Result<()> {
        let title = truncate_chars(&one_line(title), MAX_TITLE_CHARS);
        let company = truncate_chars(&one_line(company), MAX_FIELD_CHARS);
        let location = one_line(location);
        self.write(|conn| {
            let stored: String = conn
                .query_row(
                    "SELECT location FROM job WHERE portal = ?1 AND job_id = ?2",
                    params![key.portal.key(), key.id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let location = page_location(&stored, &location, MAX_FIELD_CHARS);
            conn.execute(
                "UPDATE job SET
                    title    = CASE WHEN ?3 <> '' THEN ?3 ELSE title END,
                    company  = CASE WHEN ?4 <> '' THEN ?4 ELSE company END,
                    location = CASE WHEN ?5 <> '' THEN ?5 ELSE location END
                 WHERE portal = ?1 AND job_id = ?2",
                params![key.portal.key(), key.id, title, company, location],
            )?;
            bump(conn)?;
            refresh_search(conn, key)
        })
    }

    // ------------------------------------------------------------------ Textdateien

    /// Jobs mit Volltext samt Text: nur die, deren Textdatei noch nie geschrieben wurde –
    /// oder mit `all` alle („Textdateien neu schreiben“).
    pub fn txt_jobs(&self, all: bool) -> Result<Vec<(JobRow, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {JOB_COLUMNS}, desc_text FROM job
             WHERE desc_status = 'ok' AND desc_text IS NOT NULL
               AND (?1 OR txt_written_at IS NULL)
             ORDER BY first_seen_at, portal, job_id"
        ))?;
        let rows = stmt.query_map([all], |r| {
            let text: String = r.get(JOB_COLUMN_COUNT)?;
            Ok(job_row(r)?.map(|job| (job, text)))
        })?;
        rows.map(|r| r?).collect()
    }

    /// Textdatei geschrieben – sie wird nie wieder von selbst neu angelegt, auch wenn
    /// der Nutzer sie löscht (der Matching-Skill liest alle Dateien im Ordner).
    pub fn mark_txt_written(&self, key: &JobKey, file_name: &str, now: Timestamp) -> Result<()> {
        self.conn().execute(
            "UPDATE job SET txt_name = ?3, txt_written_at = ?4 WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id, file_name, to_db(now)],
        )?;
        Ok(())
    }

    /// Namen aller Textdateien, die die App geschrieben hat (für „Ergebnisordner leeren“).
    pub fn txt_names(&self) -> Result<Vec<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT txt_name FROM job WHERE txt_name IS NOT NULL")?;
        let names = stmt.query_map([], |r| r.get(0))?;
        Ok(names.collect::<rusqlite::Result<_>>()?)
    }

    // ------------------------------------------------------------------ Schlüssel/Wert

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        kv_get(&self.conn(), key)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        kv_set(&self.conn(), key, value)
    }

    /// Änderungszähler der Jobtabelle: steigt bei jeder Änderung, die in einem Export
    /// sichtbar wäre. Exporte werden nur neu erzeugt, wenn er seit dem letzten Export
    /// gestiegen ist.
    pub fn data_rev(&self) -> Result<i64> {
        Ok(kv_get_i64(&self.conn(), "data_rev")?.unwrap_or(0))
    }
}

// ---------------------------------------------------------------------- Hilfen

const JOB_COLUMNS: &str = "portal, job_id, url, title, company, location, mail_date, mail_subject,
    gmail_id, first_seen_at, first_seen_run, desc_status, desc_short, desc_closed,
    COALESCE(LENGTH(desc_text), 0), desc_fetched_at, desc_attempts, desc_error, txt_name";
const JOB_COLUMN_COUNT: usize = 19;

/// Automatisch abrufbar: offen oder fehlgeschlagen (frühestens `?2` nach dem letzten
/// Versuch), Mail nicht älter als `?1`.
const DUE: &str = "COALESCE(mail_date, first_seen_at) >= ?1
    AND (desc_status = 'missing'
         OR (desc_status = 'failed' AND COALESCE(desc_attempted_at, 0) <= ?2))";

fn due_params(now: Timestamp, max_age: SignedDuration, retry_after: SignedDuration) -> [i64; 2] {
    [
        to_db(now.saturating_sub(max_age).unwrap_or(Timestamp::MIN)),
        to_db(now.saturating_sub(retry_after).unwrap_or(Timestamp::MIN)),
    ]
}

/// Liest eine Zeile; unbekannte Werte werden zu `Error::Corrupt` (innerer Result).
fn job_row(r: &Row<'_>) -> rusqlite::Result<Result<JobRow>> {
    let portal: String = r.get(0)?;
    let url: String = r.get(2)?;
    let status: String = r.get(11)?;
    let gmail_id: Option<String> = r.get(8)?;
    let first_seen_at: i64 = r.get(9)?;
    let (Some(portal), Ok(url), Some(desc_status), Some(first_seen_at)) = (
        Portal::from_key(&portal),
        Url::parse(&url),
        DescStatus::parse(&status),
        from_db(first_seen_at),
    ) else {
        return Ok(Err(Error::Corrupt(format!(
            "Job-Zeile {portal}/{url}/{status}"
        ))));
    };
    Ok(Ok(JobRow {
        key: JobKey {
            portal,
            id: r.get(1)?,
        },
        url,
        title: r.get(3)?,
        company: r.get(4)?,
        location: r.get(5)?,
        mail_date: r.get::<_, Option<i64>>(6)?.and_then(from_db),
        mail_subject: r.get(7)?,
        gmail_id: gmail_id.and_then(|s| s.parse().ok()),
        first_seen_at,
        first_seen_run: r.get(10)?,
        desc_status,
        desc_short: r.get(12)?,
        desc_closed: r.get(13)?,
        desc_len: r.get(14)?,
        desc_fetched_at: r.get::<_, Option<i64>>(15)?.and_then(from_db),
        desc_attempts: r.get(16)?,
        desc_error: r.get(17)?,
        txt_name: r.get(18)?,
    }))
}

/// Nimmt einen Eintrag auf. Merge-Regel für bekannte Jobs: Mail-Angaben und Erstsichtung
/// bleiben unverändert; Titel, Firma und Ort werden nur gefüllt, wenn sie leer bzw. der
/// Platzhalter sind – nie ersetzt, nur weil ein anderer Wert länger ist.
fn upsert(
    conn: &Connection,
    run: i64,
    posting: &Posting,
    mail: MailRef<'_>,
    now: Timestamp,
) -> Result<Seen> {
    let key = &posting.key;
    let known: Option<(i64, String, String, String)> = conn
        .query_row(
            "SELECT last_seen_run, title, company, location FROM job
             WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((last_run, title, company, location)) = known else {
        conn.execute(
            "INSERT INTO job (portal, job_id, url, title, company, location, mail_date,
                              mail_subject, gmail_id, first_seen_at, first_seen_run,
                              last_seen_run, search)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
            params![
                key.portal.key(),
                key.id,
                posting.url.as_str(),
                posting.title,
                posting.company,
                posting.location,
                mail.date.map(to_db),
                mail.subject,
                mail.gmail_id.map(|id| id.to_string()),
                to_db(now),
                run,
                search_text(&posting.title, &posting.company, &posting.location, ""),
            ],
        )?;
        bump(conn)?;
        return Ok(Seen::New);
    };
    // Die letzte Sichtung ist für Export und Oberfläche unsichtbar – daher kein bump().
    conn.execute(
        "UPDATE job SET last_seen_run = ?3 WHERE portal = ?1 AND job_id = ?2",
        params![key.portal.key(), key.id, run],
    )?;
    let new_title = if (title.is_empty() || title == TITLE_PLACEHOLDER) && posting.has_real_title()
    {
        posting.title.clone()
    } else {
        title.clone()
    };
    let (new_company, new_location) =
        merge_details((&company, &location), (&posting.company, &posting.location));
    if new_title != title || new_company != company || new_location != location {
        conn.execute(
            "UPDATE job SET title = ?3, company = ?4, location = ?5 WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id, new_title, new_company, new_location],
        )?;
        refresh_search(conn, key)?;
        bump(conn)?;
    }
    Ok(if last_run == run {
        Seen::DupInRun
    } else {
        Seen::KnownBefore
    })
}

/// Firma und Ort werden nur als Paar übernommen – nie aus zwei verschiedenen Mails
/// gemischt: wenn noch keine Angaben da sind, oder wenn die gespeicherte „Firma“ in
/// Wahrheit nur ein Ort war („D-20038 Hamburg“) und die neue Mail eine echte Firma nennt.
fn merge_details(stored: (&str, &str), new: (&str, &str)) -> (String, String) {
    let keep = (stored.0.to_string(), stored.1.to_string());
    let take = (new.0.to_string(), new.1.to_string());
    if new.0.is_empty() && new.1.is_empty() {
        return keep;
    }
    if stored.0.is_empty() && stored.1.is_empty() {
        return take;
    }
    let stored_has_company = !split_company_location(stored.0, stored.1).0.is_empty();
    let new_has_company = !split_company_location(new.0, new.1).0.is_empty();
    if !stored_has_company && new_has_company {
        take
    } else {
        keep
    }
}

/// Suchspalte neu berechnen (klein geschrieben, Umlaute inklusive).
fn refresh_search(conn: &Connection, key: &JobKey) -> Result<()> {
    let row: Option<(String, String, String, Option<String>)> = conn
        .query_row(
            "SELECT title, company, location, desc_text FROM job WHERE portal = ?1 AND job_id = ?2",
            params![key.portal.key(), key.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    if let Some((title, company, location, text)) = row {
        conn.execute(
            "UPDATE job SET search = ?3 WHERE portal = ?1 AND job_id = ?2",
            params![
                key.portal.key(),
                key.id,
                search_text(&title, &company, &location, text.as_deref().unwrap_or(""))
            ],
        )?;
    }
    Ok(())
}

fn search_text(title: &str, company: &str, location: &str, text: &str) -> String {
    fold(&format!("{title}\n{company}\n{location}\n{text}"))
}

/// Vergleichsform für die Suche: klein geschrieben (Unicode, also auch Ä/ä).
fn fold(text: &str) -> String {
    text.to_lowercase()
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn scan_key(portal: Portal) -> String {
    format!("last_scan:{}", portal.key())
}

fn kv_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
        .optional()?)
}

fn kv_get_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    kv_get(conn, key)?
        .map(|v| {
            v.parse()
                .map_err(|_| Error::Corrupt(format!("{key} = {v}")))
        })
        .transpose()
}

fn kv_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn bump(conn: &Connection) -> Result<()> {
    let next = kv_get_i64(conn, "data_rev")?.unwrap_or(0) + 1;
    kv_set(conn, "data_rev", &next.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portal::job_link;

    fn now() -> Timestamp {
        "2026-09-19T10:00:00Z".parse().unwrap()
    }

    fn posting(url: &str, title: &str, company: &str, location: &str) -> Posting {
        let link = job_link(url).unwrap();
        Posting::new(link.key, link.url, title, company, location)
    }

    fn mail() -> MailRef<'static> {
        MailRef {
            subject: "3 neue Jobs",
            date: Some("2026-09-18T07:00:00Z".parse().unwrap()),
            gmail_id: Some(0x1a2b),
        }
    }

    #[test]
    fn schema_is_created_once_and_reopened() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("jobs.db");
        let store = Store::open(&path).unwrap();
        let run = store.begin_run().unwrap();
        store
            .upsert_posting(
                run,
                &posting(
                    "https://www.linkedin.com/jobs/view/4123456789/",
                    "A",
                    "",
                    "",
                ),
                mail(),
                now(),
            )
            .unwrap();
        drop(store);
        let again = Store::open(&path).unwrap();
        assert_eq!(again.job_count().unwrap(), 1);
        assert_eq!(again.begin_run().unwrap(), 2);
    }

    /// Eine Datenbank aus Schema 1 (mit den beiden nie benutzten Spalten) wird beim Öffnen
    /// hochgezogen; ohne das scheiterte jeder Postfach-Abruf an „NOT NULL `alert_mail.sender`“.
    #[test]
    fn an_old_database_is_migrated_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(&format!(
            "{SCHEMA}
             DROP TABLE alert_mail;
             CREATE TABLE alert_mail (
                 mail_key       TEXT    PRIMARY KEY,
                 portal         TEXT    NOT NULL,
                 subject        TEXT    NOT NULL,
                 sender         TEXT    NOT NULL,
                 mail_date      INTEGER,
                 gmail_id       TEXT,
                 n_postings     INTEGER NOT NULL,
                 first_seen_run INTEGER NOT NULL,
                 last_seen_run  INTEGER NOT NULL
             ) WITHOUT ROWID;
             PRAGMA user_version = 1;"
        ))
        .unwrap();
        drop(old);

        let store = Store::open(&path).unwrap();
        let run = store.begin_run().unwrap();
        // Genau der Schritt, der vorher scheiterte: eine Mail ohne erkannte Einträge merken.
        let alert = AlertMail {
            key: "m1".into(),
            portal: Portal::LinkedIn,
            subject: "Keine Treffer".into(),
            sender: "LinkedIn".into(),
            date: None,
            gmail_id: Some(1),
            postings: Vec::new(),
        };
        store.record_alert(run, &alert, now()).unwrap();
        assert_eq!(store.zero_posting_mails(run).unwrap().len(), 1);
    }

    #[test]
    fn seen_counts_new_known_and_duplicates() {
        let store = Store::in_memory().unwrap();
        let run1 = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4123456789/",
            "Interim CFO",
            "Nordlicht AG",
            "Hamburg",
        );
        assert_eq!(
            store.upsert_posting(run1, &a, mail(), now()).unwrap(),
            Seen::New
        );
        // Derselbe Job in einer zweiten Mail desselben Laufs.
        assert_eq!(
            store.upsert_posting(run1, &a, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        let run2 = store.begin_run().unwrap();
        assert_eq!(
            store.upsert_posting(run2, &a, mail(), now()).unwrap(),
            Seen::KnownBefore
        );
        // Auch ein bekannter Job ist in der zweiten Mail desselben
        // Laufs eine Dublette, nicht noch einmal „schon bekannt“.
        assert_eq!(
            store.upsert_posting(run2, &a, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        // Zwei Link-Formen desselben Projekts = derselbe Job.
        let f1 = posting(
            "https://www.freelance.de/project/index.php?id=1255067",
            "SAP",
            "",
            "",
        );
        let f2 = posting(
            "https://www.freelance.de/projekte/projekt-1255067-sap",
            "SAP",
            "",
            "",
        );
        assert_eq!(
            store.upsert_posting(run2, &f1, mail(), now()).unwrap(),
            Seen::New
        );
        assert_eq!(
            store.upsert_posting(run2, &f2, mail(), now()).unwrap(),
            Seen::DupInRun
        );
        assert_eq!(store.job_count().unwrap(), 2);
    }

    #[test]
    fn merge_fills_gaps_but_never_replaces() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let url = "https://www.linkedin.com/jobs/view/4123456789/";
        store
            .upsert_posting(run, &posting(url, "", "", "Hamburg"), mail(), now())
            .unwrap();
        let later = MailRef {
            subject: "Andere Mail",
            date: None,
            gmail_id: None,
        };
        store
            .upsert_posting(
                run,
                &posting(url, "Interim CFO", "Nordlicht AG", "Berlin (anderer Wert)"),
                later,
                now(),
            )
            .unwrap();
        store
            .upsert_posting(
                run,
                &posting(url, "Interim CFO (m/w/d) – längerer Titel", "X", "Y"),
                later,
                now(),
            )
            .unwrap();
        let job = store.job(&job_link(url).unwrap().key).unwrap().unwrap();
        assert_eq!(job.title, "Interim CFO"); // Platzhalter ersetzt, dann nie wieder
        // Firma und Ort als Paar aus derselben Mail: Die erste nannte nur einen Ort, die
        // zweite Firma und Ort – nie gemischt; die dritte ändert nichts.
        assert_eq!(job.company, "Nordlicht AG");
        assert_eq!(job.location, "Berlin (anderer Wert)");
        assert_eq!(job.mail_subject, "3 neue Jobs"); // Erstsichtung bleibt
        assert_eq!(job.gmail_id, Some(0x1a2b));
    }

    /// Die „Firma“ war nur ein Ort (freelance.de nennt hinter dem Titel oft
    /// nur die Stadt) – eine spätere Mail mit echter Firma ersetzt das Paar; eine echte Firma
    /// wird nie durch eine andere ersetzt.
    #[test]
    fn a_real_company_replaces_a_place_only_pair() {
        assert_eq!(
            merge_details(
                ("D-20038 Hamburg", ""),
                ("Muster Consulting GmbH", "D-20038 Hamburg")
            ),
            (
                "Muster Consulting GmbH".to_string(),
                "D-20038 Hamburg".to_string()
            )
        );
        assert_eq!(
            merge_details(("Firma A", "Köln"), ("Firma B", "Berlin")),
            ("Firma A".to_string(), "Köln".to_string())
        );
        assert_eq!(
            merge_details(("Firma A", ""), ("", "")),
            ("Firma A".to_string(), String::new())
        );
    }

    #[test]
    fn fetch_lifecycle_and_queue_order() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let old_mail = MailRef {
            subject: "alt",
            date: Some("2026-08-01T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let new_mail = MailRef {
            subject: "neu",
            date: Some("2026-09-18T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let mid_mail = MailRef {
            subject: "mittel",
            date: Some("2026-09-10T07:00:00Z".parse().unwrap()),
            gmail_id: None,
        };
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "B",
            "",
            "",
        );
        let c = posting(
            "https://www.linkedin.com/jobs/view/4000000003/",
            "C",
            "",
            "",
        );
        let d = posting(
            "https://www.linkedin.com/jobs/view/4000000004/",
            "D",
            "",
            "",
        );
        store.upsert_posting(run, &a, mid_mail, now()).unwrap();
        store.upsert_posting(run, &b, new_mail, now()).unwrap();
        store.upsert_posting(run, &c, old_mail, now()).unwrap(); // älter als 30 Tage
        store.upsert_posting(run, &d, new_mail, now()).unwrap();
        let month = SignedDuration::from_hours(30 * 24);
        let half_day = SignedDuration::from_hours(12);

        // d schlägt jetzt fehl → steht vorerst nicht in der Warteschlange.
        assert_eq!(
            store.record_failed(&d.key, "leer", now()).unwrap(),
            DescStatus::Failed
        );
        let queue: Vec<_> = store
            .fetch_queue(now(), month, half_day)
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(queue, ["B", "A"]); // neueste Mail zuerst, alte Mail nicht automatisch

        // 13 h später: d ist wieder dran, aber nach den offenen.
        let later = now() + SignedDuration::from_hours(13);
        let queue: Vec<_> = store
            .fetch_queue(later, month, half_day)
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(queue, ["B", "A", "D"]);

        assert_eq!(
            store.record_failed(&d.key, "leer", later).unwrap(),
            DescStatus::Failed
        );
        assert_eq!(
            store.record_failed(&d.key, "leer", later).unwrap(),
            DescStatus::Unfetchable
        );
        store
            .record_text(&b.key, "Volltext B", false, false, now())
            .unwrap();
        store.record_gone(&a.key, now()).unwrap();
        let later2 = later + SignedDuration::from_hours(24);
        assert!(
            store
                .fetch_queue(later2, month, half_day)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store.description(&b.key).unwrap().as_deref(),
            Some("Volltext B")
        );
        assert_eq!(store.description(&a.key).unwrap(), None);
    }

    /// Ein Fehlschlag oder „weg“ nach erfolgreichem Abruf stuft den Job
    /// nicht zurück – der Text bleibt, nichts wird erneut geholt.
    #[test]
    fn success_is_never_downgraded() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting("https://www.freelancermap.de/nproj/12345.html", "A", "", "");
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        store
            .record_text(&a.key, "Volltext", false, false, now())
            .unwrap();
        let rev = store.data_rev().unwrap();
        assert_eq!(
            store.record_failed(&a.key, "leer", now()).unwrap(),
            DescStatus::Ok
        );
        store.record_gone(&a.key, now()).unwrap();
        let job = store.job(&a.key).unwrap().unwrap();
        assert_eq!((job.desc_status, job.desc_attempts), (DescStatus::Ok, 0));
        assert_eq!(
            store.description(&a.key).unwrap().as_deref(),
            Some("Volltext")
        );
        assert_eq!(store.data_rev().unwrap(), rev);
    }

    #[test]
    fn txt_written_exactly_once() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        assert!(store.txt_jobs(false).unwrap().is_empty());
        store
            .record_text(&a.key, "Text", false, false, now())
            .unwrap();
        let pending = store.txt_jobs(false).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "Text");
        store
            .mark_txt_written(&a.key, "20260918_LinkedIn_A_4000000001.txt", now())
            .unwrap();
        assert!(store.txt_jobs(false).unwrap().is_empty());
        assert_eq!(
            store.txt_names().unwrap(),
            ["20260918_LinkedIn_A_4000000001.txt"]
        );
        // Neu schreiben sieht alle – ohne die Marke anzufassen.
        assert_eq!(store.txt_jobs(true).unwrap().len(), 1);
        assert!(store.txt_jobs(false).unwrap().is_empty());
    }

    /// Fehlergründe stammen oft aus der Seite (Umleitungsziel, Skriptfehler): gespeichert
    /// wird eine kurze Zeile – sie reist bis in die Ereignisse an die Oberfläche.
    #[test]
    fn failure_reasons_are_short_and_flat() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting("https://www.freelancermap.de/nproj/12345.html", "A", "", "");
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        let reason = format!(
            "unerwartete Umleitung nach /{}\nzweite Zeile",
            "x".repeat(20_000)
        );
        store.record_failed(&a.key, &reason, now()).unwrap();
        let error = store.job(&a.key).unwrap().unwrap().desc_error.unwrap();
        assert_eq!(error.chars().count(), MAX_ERROR_CHARS);
        assert!(!error.contains('\n'));
    }

    /// Eine Alert-Mail wird ganz oder gar nicht übernommen.
    #[test]
    fn an_alert_mail_is_one_change() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let alert = AlertMail {
            key: "gm:1".into(),
            portal: Portal::LinkedIn,
            subject: "2 neue Jobs".into(),
            sender: "LinkedIn".into(),
            date: None,
            gmail_id: Some(1),
            postings: vec![
                posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000002/",
                    "B",
                    "",
                    "",
                ),
            ],
        };
        assert_eq!(
            store.record_alert(run, &alert, now()).unwrap(),
            [Seen::New, Seen::DupInRun, Seen::New]
        );
        assert_eq!(store.job_count().unwrap(), 2);
        // Scheitert eine Anweisung, bleibt nichts von der Mail zurück.
        store
            .conn()
            .execute_batch(
                "CREATE TRIGGER no_b BEFORE INSERT ON job WHEN NEW.job_id = '4000000003'
                 BEGIN SELECT RAISE(ABORT, 'Test'); END;",
            )
            .unwrap();
        let failing = AlertMail {
            key: "gm:2".into(),
            postings: vec![
                posting(
                    "https://www.linkedin.com/jobs/view/4000000004/",
                    "D",
                    "",
                    "",
                ),
                posting(
                    "https://www.linkedin.com/jobs/view/4000000003/",
                    "C",
                    "",
                    "",
                ),
            ],
            ..alert
        };
        assert!(store.record_alert(run, &failing, now()).is_err());
        assert_eq!(store.job_count().unwrap(), 2);
        assert!(store.zero_posting_mails(run).unwrap().is_empty());
        let mails: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM alert_mail", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mails, 1);
    }

    #[test]
    fn search_is_case_insensitive_with_umlauts_and_wildcards_are_literal() {
        let store = Store::in_memory().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "Überwachung SAP",
            "Müller GmbH",
            "Köln",
        );
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "100% Remote",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        store.upsert_posting(run, &b, mail(), now()).unwrap();
        let find = |q: &str| {
            store
                .jobs(&JobFilter {
                    search: Some(q.into()),
                    ..JobFilter::default()
                })
                .unwrap()
                .into_iter()
                .map(|j| j.title)
                .collect::<Vec<_>>()
        };
        assert_eq!(find("überwachung"), ["Überwachung SAP"]);
        assert_eq!(find("MÜLLER"), ["Überwachung SAP"]);
        assert_eq!(find("100%"), ["100% Remote"]);
        assert!(find("%").iter().all(|t| t == "100% Remote"));
        assert_eq!(find("  ").len(), 2); // leere Suche = alles
        store
            .record_text(
                &a.key,
                "Wir suchen einen Projektleiter",
                false,
                false,
                now(),
            )
            .unwrap();
        assert_eq!(find("projektleiter"), ["Überwachung SAP"]);
    }

    #[test]
    fn filter_by_run_and_newest_first() {
        let store = Store::in_memory().unwrap();
        let run1 = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run1, &a, mail(), now()).unwrap();
        let run2 = store.begin_run().unwrap();
        let b = posting(
            "https://www.linkedin.com/jobs/view/4000000002/",
            "B",
            "",
            "",
        );
        let later = now() + SignedDuration::from_mins(1);
        store.upsert_posting(run2, &b, mail(), later).unwrap();
        store.upsert_posting(run2, &a, mail(), later).unwrap(); // bekannt, zählt nicht als neu
        let only_new: Vec<_> = store
            .jobs(&JobFilter {
                first_seen_run: Some(run2),
                ..JobFilter::default()
            })
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(only_new, ["B"]);
        let all: Vec<_> = store
            .jobs(&JobFilter::default())
            .unwrap()
            .into_iter()
            .map(|j| j.title)
            .collect();
        assert_eq!(all, ["B", "A"]);
    }

    #[test]
    fn data_rev_changes_only_with_visible_content() {
        let store = Store::in_memory().unwrap();
        let v0 = store.data_rev().unwrap();
        let run = store.begin_run().unwrap();
        let a = posting(
            "https://www.linkedin.com/jobs/view/4000000001/",
            "A",
            "",
            "",
        );
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        let v1 = store.data_rev().unwrap();
        assert!(v1 > v0);
        // Dieselbe Mail noch einmal: nichts ändert sich, also auch kein neuer Export.
        store.upsert_posting(run, &a, mail(), now()).unwrap();
        assert_eq!(store.data_rev().unwrap(), v1);
        store
            .upsert_posting(
                run,
                &posting(
                    "https://www.linkedin.com/jobs/view/4000000001/",
                    "A",
                    "Firma",
                    "",
                ),
                mail(),
                now(),
            )
            .unwrap();
        let v2 = store.data_rev().unwrap();
        assert!(v2 > v1);
        store
            .record_text(&a.key, "Text", false, false, now())
            .unwrap();
        assert!(store.data_rev().unwrap() > v2);
        let v3 = store.data_rev().unwrap();
        store.mark_txt_written(&a.key, "x.txt", now()).unwrap();
        assert_eq!(store.data_rev().unwrap(), v3);
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        // Nicht „beschädigt“ – sonst legte jemand eine gesunde Datenbank beiseite.
        assert!(matches!(Store::open(&path), Err(Error::NewerSchema(99))));
    }
}
