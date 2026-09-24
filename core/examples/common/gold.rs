//! The private gold folder (`core/tests/fixtures/private/gold`, ignored by git): `jobs.json`,
//! one TXT per job (`<portal>_<id>.txt`, TXT contract format), `labels.json` from the blind
//! labelers, `labeling/` for them and the `report.md` of `match_eval`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The gold folder below the crate root.
pub const GOLD_DIR: &str = "tests/fixtures/private/gold";
pub const JOBS_FILE: &str = "jobs.json";
pub const LABELS_FILE: &str = "labels.json";
pub const REPORT_FILE: &str = "report.md";
pub const LABELING_DIR: &str = "labeling";
pub const RUBRIC_FILE: &str = "RUBRIC.md";
/// Longest file stem of a job (portal, underscore, id).
const MAX_STEM_CHARS: usize = 120;

pub fn gold_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLD_DIR)
}

/// The invented profiles of the synthetic corpus: `sample_profile*.json`, sorted.
pub fn default_profiles() -> std::io::Result<Vec<PathBuf>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/matching");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("sample_profile"))
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    found.sort();
    Ok(found)
}

/// The profile's key in `labels.json`: its file name (`sample_profile_senior.json`).
pub fn profile_key(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The profile's folder name under `labeling/`: its file stem.
pub fn profile_stem(path: &Path) -> String {
    path.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// One exported job. `file` is the stem of its TXT and labeling files and the key the
/// labelers use; `key` is the app's `portal:id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldJob {
    pub key: String,
    pub file: String,
    pub portal: String,
    pub url: String,
    pub title: String,
    pub company: String,
    /// The location as the portal or mail gave it (what the engine gets).
    pub location: String,
    /// RFC 3339; `None` when the alert mail had no date.
    pub mail_date: Option<String>,
    pub first_seen_at: String,
    /// `ok` or `teaser`.
    pub desc_status: String,
    /// The page's structured facts (`desc_facts`), if any.
    pub facts: Option<Value>,
}

impl GoldJob {
    pub fn is_teaser(&self) -> bool {
        self.desc_status == "teaser"
    }

    pub fn txt_name(&self) -> String {
        format!("{}.txt", self.file)
    }
}

/// `<portal>_<id>`, with every character outside `[A-Za-z0-9._-]` replaced by `_`.
pub fn file_stem(portal: &str, id: &str) -> String {
    let raw = format!("{portal}_{id}");
    raw.chars()
        .take(MAX_STEM_CHARS)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn read_jobs(path: &Path) -> Result<Vec<GoldJob>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Jobs sorted by key, pretty JSON.
pub fn jobs_json(jobs: &[GoldJob]) -> String {
    let mut sorted = jobs.to_vec();
    sorted.sort_by(|a, b| a.key.cmp(&b.key));
    let mut out = serde_json::to_string_pretty(&sorted).expect("serialisable");
    out.push('\n');
    out
}

/// A job file in the TXT contract format: the header values and the body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TxtFile {
    pub title: String,
    pub company: String,
    pub location: String,
    pub source: String,
    pub body: String,
}

/// Reads the header lines up to the first empty line; the rest (trimmed) is the body.
pub fn parse_txt(content: &str) -> TxtFile {
    let content = content.replace("\r\n", "\n");
    let (header, body) = content.split_once("\n\n").unwrap_or((&content, ""));
    let field = |name: &str| {
        header
            .lines()
            .find_map(|l| l.strip_prefix(name).map(|v| v.trim().to_owned()))
            .unwrap_or_default()
    };
    TxtFile {
        title: field("Titel:"),
        company: field("Unternehmen:"),
        location: field("Ort:"),
        source: field("Quelle:"),
        body: body.trim().to_owned(),
    }
}

/// Top-level keys with contact or personal data (compared case-insensitively). `name` only
/// counts at the top level: `methoden_tools[].name` is a competence.
const TOP_CONTACT_KEYS: &[&str] = &["name", "vorname", "nachname", "first_name", "last_name"];
/// Keys with contact data at any depth.
const CONTACT_KEYS: &[&str] = &[
    "address",
    "adresse",
    "age",
    "alter",
    "anschrift",
    "birthday",
    "contact",
    "e-mail",
    "email",
    "foto",
    "geburtsdatum",
    "handy",
    "homepage",
    "kontakt",
    "kundennummer",
    "linkedin",
    "mitgliedsnummer",
    "mobil",
    "mobile",
    "phone",
    "photo",
    "plz",
    "profilbild",
    "strasse",
    "street",
    "telefon",
    "website",
    "wohnort",
    "xing",
];

/// The profile without contact data (name, mail, phone, address, links, photo).
pub fn strip_contact(profile: &Value) -> Value {
    fn strip(value: &Value, top: bool) -> Value {
        match value {
            Value::Object(map) => {
                let kept: Map<String, Value> = map
                    .iter()
                    .filter(|(k, _)| {
                        let k = k.to_lowercase();
                        let contact = CONTACT_KEYS.contains(&k.as_str())
                            || (top && TOP_CONTACT_KEYS.contains(&k.as_str()));
                        !contact
                    })
                    .map(|(k, v)| (k.clone(), strip(v, false)))
                    .collect();
                Value::Object(kept)
            }
            Value::Array(items) => Value::Array(items.iter().map(|v| strip(v, false)).collect()),
            other => other.clone(),
        }
    }
    strip(profile, true)
}

/// What a blind labeler sees of an ad: no link, no fetch date, no score, no app output.
pub struct AdView<'a> {
    pub title: &'a str,
    pub company: &'a str,
    pub location: &'a str,
    pub portal: &'a str,
    pub teaser: bool,
    pub facts: Option<&'a Value>,
    pub body: &'a str,
}

/// One labeling file: the profile JSON (already without contact data) and the ad.
pub fn labeling_markdown(profile: &Value, ad: &AdView<'_>) -> String {
    let mut out = String::from("# Profile\n\n```json\n");
    out.push_str(&serde_json::to_string_pretty(profile).expect("serialisable"));
    out.push_str("\n```\n\n# Job ad\n\n");
    let _ = writeln!(out, "- Title: {}", ad.title);
    let _ = writeln!(out, "- Company: {}", ad.company);
    let _ = writeln!(out, "- Location: {}", ad.location);
    let _ = writeln!(out, "- Portal: {}", ad.portal);
    if ad.teaser {
        out.push_str("- Only a short teaser of the ad was visible.\n");
    }
    if let Some(Value::Object(facts)) = ad.facts {
        for (key, value) in facts {
            let value = match value {
                Value::String(s) => s.clone(),
                Value::Array(items) => items
                    .iter()
                    .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                    .collect::<Vec<_>>()
                    .join(", "),
                other => other.to_string(),
            };
            let _ = writeln!(out, "- Page fact {key}: {value}");
        }
    }
    out.push_str("\n## Text\n\n");
    out.push_str(ad.body);
    out.push('\n');
    out
}

/// The grading rubric for the blind labelers (`labeling/RUBRIC.md`).
pub const RUBRIC: &str = r#"# Grading rubric

Every folder in `labeling/` belongs to one consultant profile (folder name = profile file stem).
Every file in it holds that profile as JSON and one job ad. Judge each ad against that profile
only, the way the consultant would when reading the ad. Use only what the file shows: no other
files, no web search, no guessing about the company.

## Grade (0-3)

- 3 = apply immediately: the role is squarely in the profile's field and level, and its core
  requirements are covered by the profile.
- 2 = look closer: a plausible fit worth reading in full; some requirements are open or unclear.
- 1 = in the field but rather not: same domain, but focus, level or setting fit poorly.
- 0 = off-field: another profession or domain.

Give the grade for the fit of the content, also when the ad is excluded (below).

## Exclusion (`excluded: true`)

Only on clear wording in the ad, and only for a criterion the profile actually sets
(`harte_kriterien` and related keys). A criterion the profile does not set never excludes.

- Temporary agency work (ANÜ, Arbeitnehmerüberlassung) when the profile excludes that contract type.
- The place of work is in a country outside the profile's countries (`laender`), unless the
  role is fully remote and the profile allows remote work from abroad (`remote_ausserhalb_erlaubt`).
  A foreign headquarters or client alone is not the place of work.
- Interim or freelance role: the upper bound of the stated EUR day rate is below the profile's
  minimum (`min_tagessatz`, else `einsatzpraeferenzen.tagessatz_ab`); an hourly rate counts x 8.
  A rate in another currency is not an exclusion.
- Permanent role: the upper bound of the stated annual salary is below the profile's minimum
  (`min_jahresgehalt`), only when the ad states a salary.
- Permanent role outside the profile's region (`festanstellung_orte`), unless the ad states a
  remote share of at least the profile's remote minimum (`festanstellung_remote_min`) or fully remote.
- The target seniority is clearly too junior: the ad asks for fewer years than the profile's
  minimum (`zielprofil_min_jahre`), as a closed range below it or as a minimum below it without
  a senior title (Senior, Lead, Principal, Head, Director, Leitung).

Everything else is not an exclusion: missing or vague information, a later or unclear start,
hybrid wording, a missing or different degree, an agency without details, a teaser without
details. Grade such ads normally and name the doubt in `reason`.

## Output

One JSON file per labeler in this shape (profile key = folder name + `.json`, job key = file
name without `.md`):

```json
{
  "sample_profile_senior.json": {
    "linkedin_4012345678": {
      "grade": 2,
      "excluded": false,
      "reason": "One short English sentence.",
      "quote": "The few words of the ad that decide it, or empty."
    }
  }
}
```
"#;
