//! Alert-Mails als lokale Testdaten sichern (vom Nutzer selbst auszuführen – fragt
//! Gmail-Adresse und App-Passwort ab):
//!
//! ```text
//! cargo run -p jobalert-core --example dump_alerts
//! ```
//!
//! Liest nur (wie die App) alle Treffer der App-Suche der letzten 30 Tage und legt jede
//! Mail unverändert als `core/tests/fixtures/private/mails/<Gmail-ID>.eml` ab – auch die,
//! die die App nicht als Alert erkennt (für den Vergleich mit dem Altprogramm). Der Ordner
//! ist von Git ausgeschlossen. Gedruckt werden nur Zahlen je Portal.
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;

use jiff::ToSpan as _;
use jobalert_core::mail::imap::{Credentials, Gmail, MailSource as _};
use jobalert_core::mail::{MailKind, classify_mail};
use jobalert_core::portal::Portal;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    jobalert_core::install_crypto();
    print!("Gmail-Adresse: ");
    std::io::stdout().flush()?;
    let mut user = String::new();
    std::io::stdin().read_line(&mut user)?;
    let credentials = Credentials::new(
        &user,
        &rpassword::prompt_password("App-Passwort (Eingabe unsichtbar): ")?,
    );
    let mut gmail = Gmail::connect(&credentials, CancellationToken::new()).await?;
    drop(credentials);

    let since = jiff::Zoned::now().date().checked_sub(30.days())?;
    let uids = gmail.search(Some(since), &Portal::ALL).await?;
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/private/mails");
    std::fs::create_dir_all(&dir)?;
    let mut per_portal = BTreeMap::<&str, (usize, usize)>::new();
    let (mut other, mut defective, mut saved) = (0, 0, 0);
    for chunk in uids.chunks(jobalert_core::mail::imap::BATCH) {
        for raw in gmail.fetch(chunk).await? {
            let name = raw
                .gmail_id
                .map_or_else(|| format!("uid_{saved}"), |id| format!("{id:x}"));
            std::fs::write(dir.join(format!("{name}.eml")), &raw.bytes)?;
            saved += 1;
            match classify_mail(&raw, &Portal::ALL) {
                MailKind::Alert(alert) => {
                    let entry = per_portal.entry(alert.portal.label()).or_default();
                    entry.0 += 1;
                    entry.1 += alert.postings.len();
                }
                MailKind::Other => other += 1,
                MailKind::Defective => defective += 1,
            }
        }
    }
    gmail.close().await;
    println!("Treffer der Suche: {}", uids.len());
    for (portal, (mails, postings)) in &per_portal {
        println!("{portal}: {mails} Alert-Mails, {postings} Einträge");
    }
    println!("keine Alerts: {other}, unlesbar: {defective}");
    println!("{saved} Mails gespeichert in {}", dir.display());
    Ok(())
}
