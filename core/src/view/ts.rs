//! TypeScript types of the IPC contract, generated from the Rust types (ts-rs, tests only).
//!
//! `cargo test -p jobalert-core ipc_types` regenerates `ui/src/lib/ipc/types/` and fails
//! while a file changed - commit the result. The command map `commands.ts` next to them is
//! written by `core/tests/contract.rs` from the command table in `src-tauri`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ts_rs::{Config, TS, TypeVisitor};

use crate::error::{ErrorInfo, ErrorKind, InvalidInput};
use crate::fetch::PortalHealth;
use crate::fetch::policy::PauseReason;
use crate::model::{AppStatus, Band, MatchStatus, Notice};
use crate::pipeline::{
    ExportSummary, NewJobs, Outcome, PortalSummary, RunEvent, RunKind, RunKindName, RunRequest,
    RunSnapshot, RunSummary, ScanCounts, ScoreSummary, StatusCode, Step,
};
use crate::portal::{JobKey, Portal};
use crate::view::{
    AppState, ClearedTxt, DetailState, EmptyAlert, Evidence, Highlight, JobCounts, JobDetail,
    JobFacet, JobMail, JobMatch, JobPage, JobQuery, JobSort, JobView, Mailbox, MatchDetail,
    OpenTarget, Platform, PortalLogin, PortalNew, PortalPatch, PortalState, ProfileInfo,
    ProfileQuality, ProfileUnderstanding, Quota, Reason, ReasonKind, ReasonWeight, ResetSummary,
    Risk, SettingsPatch, SettingsView, TextRange, VaultKind, WorkMode,
};

/// A portal key.
#[derive(TS)]
#[ts(rename = "Portal")]
#[expect(
    dead_code,
    reason = "shadow of `Portal` for the TypeScript generator only"
)]
enum PortalTs {
    #[ts(rename = "linkedin")]
    LinkedIn,
    #[ts(rename = "freelance")]
    FreelanceDe,
    #[ts(rename = "freelancermap")]
    Freelancermap,
}

/// Identity of a job across all mails and runs.
#[derive(TS)]
#[ts(rename = "JobKey")]
#[expect(
    dead_code,
    reason = "shadow of `JobKey` for the TypeScript generator only"
)]
struct JobKeyTs {
    portal: Portal,
    id: String,
}

/// `Portal` and `JobKey` live in `portal.rs` without the derive; their TypeScript comes from
/// the shadows above (a test checks that the JSON agrees).
macro_rules! shadow {
    ($real:ty => $shadow:ty) => {
        impl TS for $real {
            type WithoutGenerics = $real;
            type OptionInnerType = $real;
            fn docs() -> Option<String> {
                <$shadow>::docs()
            }
            fn ident(cfg: &Config) -> String {
                <$shadow>::ident(cfg)
            }
            fn name(cfg: &Config) -> String {
                <$shadow>::name(cfg)
            }
            fn inline(cfg: &Config) -> String {
                <$shadow>::inline(cfg)
            }
            fn decl(cfg: &Config) -> String {
                <$shadow>::decl(cfg)
            }
            fn decl_concrete(cfg: &Config) -> String {
                <$shadow>::decl_concrete(cfg)
            }
            fn visit_dependencies(v: &mut impl TypeVisitor)
            where
                Self: 'static,
            {
                <$shadow>::visit_dependencies(v);
            }
            fn output_path() -> Option<PathBuf> {
                <$shadow>::output_path()
            }
        }
    };
}

shadow!(Portal => PortalTs);
shadow!(JobKey => JobKeyTs);

/// Generated file name -> content, and the files every type depends on.
struct Generated {
    cfg: Config,
    by_name: BTreeMap<String, String>,
    needed: Vec<String>,
}

impl Generated {
    fn add<T: TS + 'static>(&mut self) {
        let path = T::output_path().expect("exportable type");
        let name = path.to_string_lossy().replace('\\', "/");
        let content = T::export_to_string(&self.cfg).expect("TypeScript for the type");
        for dep in T::dependencies(&self.cfg) {
            self.needed
                .push(dep.output_path.to_string_lossy().replace('\\', "/"));
        }
        self.by_name.insert(name, content);
    }
}

/// Every type of the contract.
fn contract() -> BTreeMap<String, String> {
    // Integers are JSON numbers, never `bigint`.
    let mut f = Generated {
        cfg: Config::new().with_large_int("number"),
        by_name: BTreeMap::new(),
        needed: Vec::new(),
    };
    f.add::<Portal>();
    f.add::<JobKey>();
    f.add::<ErrorKind>();
    f.add::<ErrorInfo>();
    f.add::<InvalidInput>();
    f.add::<MatchStatus>();
    f.add::<AppStatus>();
    f.add::<Band>();
    f.add::<Notice>();
    f.add::<PauseReason>();
    f.add::<PortalHealth>();
    f.add::<WorkMode>();
    f.add::<DetailState>();
    f.add::<JobMatch>();
    f.add::<JobView>();
    f.add::<ReasonKind>();
    f.add::<ReasonWeight>();
    f.add::<Evidence>();
    f.add::<TextRange>();
    f.add::<Reason>();
    f.add::<Highlight>();
    f.add::<MatchDetail>();
    f.add::<JobMail>();
    f.add::<JobDetail>();
    f.add::<JobFacet>();
    f.add::<JobSort>();
    f.add::<JobQuery>();
    f.add::<PortalNew>();
    f.add::<JobCounts>();
    f.add::<JobPage>();
    f.add::<EmptyAlert>();
    f.add::<Platform>();
    f.add::<VaultKind>();
    f.add::<Mailbox>();
    f.add::<SettingsView>();
    f.add::<SettingsPatch>();
    f.add::<PortalPatch>();
    f.add::<PortalLogin>();
    f.add::<Risk>();
    f.add::<Quota>();
    f.add::<PortalState>();
    f.add::<ProfileQuality>();
    f.add::<ProfileUnderstanding>();
    f.add::<ProfileInfo>();
    f.add::<ResetSummary>();
    f.add::<AppState>();
    f.add::<OpenTarget>();
    f.add::<ClearedTxt>();
    f.add::<RunRequest>();
    f.add::<RunKind>();
    f.add::<RunKindName>();
    f.add::<Step>();
    f.add::<StatusCode>();
    f.add::<RunEvent>();
    f.add::<Outcome>();
    f.add::<ScanCounts>();
    f.add::<PortalSummary>();
    f.add::<ScoreSummary>();
    f.add::<NewJobs>();
    f.add::<ExportSummary>();
    f.add::<RunSummary>();
    f.add::<RunSnapshot>();
    let missing: Vec<&String> = f
        .needed
        .iter()
        .filter(|n| !f.by_name.contains_key(*n))
        .collect();
    assert!(
        missing.is_empty(),
        "types used but not exported: {missing:?}"
    );
    let mut barrel = String::new();
    for file in f.by_name.keys() {
        let module = file.trim_end_matches(".ts");
        let _ = writeln!(barrel, "export type {{ {module} }} from \"./{module}\";");
    }
    f.by_name.insert(
        "index.ts".into(),
        format!(
            "// Generated by core/src/view/ts.rs (cargo test -p jobalert-core ipc_types). Do not edit.\n\
             {barrel}export type {{ Commands }} from \"./commands\";\n"
        ),
    );
    f.by_name
}

pub(crate) fn types_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/ipc/types")
}

/// Regenerates the TypeScript types; fails while the committed files differ.
#[test]
fn ipc_types_are_generated_and_committed() {
    let dir = types_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let expected = contract();
    let mut changed = Vec::new();
    for (name, content) in &expected {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            std::fs::write(&path, content).unwrap();
            changed.push(name.clone());
        }
    }
    // Files of types that no longer exist go (the command map belongs to contract.rs).
    for entry in std::fs::read_dir(&dir).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        let is_ts = Path::new(&name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ts"));
        if is_ts && name != "commands.ts" && !expected.contains_key(&name) {
            std::fs::remove_file(dir.join(&name)).unwrap();
            changed.push(format!("{name} (removed)"));
        }
    }
    assert!(
        changed.is_empty(),
        "regenerated TypeScript types - commit them: {changed:?}"
    );
}

/// The shadows say what the real types serialise to.
#[test]
fn the_shadows_match_the_json() {
    let cfg = Config::new();
    for portal in Portal::ALL {
        let json = serde_json::to_string(&portal).unwrap();
        assert!(Portal::inline(&cfg).contains(&json), "{json}");
    }
    let key = crate::portal::job_link("https://www.linkedin.com/jobs/view/4123456789/")
        .unwrap()
        .key;
    let json = serde_json::to_value(&key).unwrap();
    let fields: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert_eq!(fields, ["id", "portal"]);
    let decl = JobKey::decl(&cfg);
    assert!(
        decl.contains("portal: Portal") && decl.contains("id: string"),
        "{decl}"
    );
}
