//! IT projects and PMO: delivery, testing, data and requirements.
//!
//! external contract - do not translate.

use super::Domain;

pub(crate) const DOMAIN: Domain = Domain {
    name: "itProject",
    triggers: &[
        "agil",
        "anforderungs",
        "confluence",
        "cutover",
        "datenmigration",
        "devops",
        "go-live",
        "hypercare",
        "it-",
        "jira",
        "pmo",
        "requirements",
        "rollout",
        "schnittstell",
        "scrum",
        "software",
        "testmanagement",
    ],
    generic: &[],
    concepts: &[
        ("data migration", "datenmigration"),
        ("test management", "testmanagement"),
        ("master data", "stammdat"),
        ("project management office", "pmo"),
        ("roll out", "rollout"),
        ("go live", "go-live"),
        ("produktivsetzung", "go-live"),
    ],
};
