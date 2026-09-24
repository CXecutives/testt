//! Save alert mails as local test data (run by the user; asks for the Gmail address and the
//! app password):
//!
//! ```text
//! cargo run -p jobalert-core --example dump_alerts
//! ```
//!
//! Read-only (like the app): takes every hit of the app's search of the last 30 days and
//! stores each mail unchanged as `core/tests/fixtures/private/mails/<Gmail-ID>.eml` - also
//! those the app does not recognise as an alert (to compare with the old program). The
//! folder is excluded from Git. Only counts per portal are printed.
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
    print!("Gmail address: ");
    std::io::stdout().flush()?;
    let mut user = String::new();
    std::io::stdin().read_line(&mut user)?;
    let credentials = Credentials::new(
        &user,
        &rpassword::prompt_password("App password (input hidden): ")?,
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
    println!("Search hits: {}", uids.len());
    for (portal, (mails, postings)) in &per_portal {
        println!("{portal}: {mails} alert mails, {postings} postings");
    }
    println!("no alerts: {other}, unreadable: {defective}");
    println!("{saved} mails saved in {}", dir.display());
    Ok(())
}
