//! Blind labels (`gold/labels.json`):
//! `{ "<profile file name>": { "<job key>": { "grade": 0-3, "excluded": bool, "reason": "...", "quote": "..." } } }`.
//! A profile may also be given by its file stem, a job by its app key `portal:id`.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::gold::GoldJob;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub grade: u8,
    #[serde(default)]
    pub excluded: bool,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub quote: String,
}

/// Profile key -> job key -> label.
pub type Labels = BTreeMap<String, BTreeMap<String, Label>>;

/// Parses and checks `labels.json`; errors name the profile and job, never label text.
pub fn parse_labels(text: &str) -> Result<Labels, String> {
    let labels: Labels = serde_json::from_str(text).map_err(|e| format!("labels.json: {e}"))?;
    for (profile, jobs) in &labels {
        for (job, label) in jobs {
            if label.grade > 3 {
                return Err(format!(
                    "labels.json: {profile} / {job}: grade {} is not 0-3",
                    label.grade
                ));
            }
        }
    }
    Ok(labels)
}

/// The labels of one profile, found by file name (`x.json`) or stem (`x`).
pub fn for_profile<'a>(labels: &'a Labels, file_name: &str) -> Option<&'a BTreeMap<String, Label>> {
    let stem = file_name.strip_suffix(".json").unwrap_or(file_name);
    labels
        .get(file_name)
        .or_else(|| labels.get(stem))
        .or_else(|| labels.get(&format!("{stem}.json")))
}

/// The label of a job, found by file stem (`portal_id`) or app key (`portal:id`).
pub fn for_job<'a>(labels: &'a BTreeMap<String, Label>, job: &GoldJob) -> Option<&'a Label> {
    labels.get(&job.file).or_else(|| labels.get(&job.key))
}

/// Label keys of a profile that match no exported job.
pub fn unknown_jobs(labels: &BTreeMap<String, Label>, jobs: &[GoldJob]) -> usize {
    labels
        .keys()
        .filter(|k| !jobs.iter().any(|j| &j.file == *k || &j.key == *k))
        .count()
}
