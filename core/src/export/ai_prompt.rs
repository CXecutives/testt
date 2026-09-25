//! The prompts for any AI chat the user likes: a deep analysis of one job, or one comparison
//! of the best current matches. The app sends nothing itself and needs no API key; for normal
//! use they replace the optional job-matching skill and give the assistant everything its
//! brief gives and more. They address the assistant as "du" without naming a product and
//! carry, in this order:
//!
//! 1. the task (role and goal),
//! 2. the profile without the consultant's name and contact data (the filter shared with the
//!    skill, [`super::personal`]) and a glossary of the profile keys it holds,
//! 3. the ad: its key facts (each one the app did not find is said to be missing), its text
//!    status (full, teaser, very short, none) and its text,
//! 4. the app's own assessment, marked as a machine pre-assessment to check, not to copy:
//!    score and band or the exclusion, every hard criterion with the profile's threshold and
//!    the ad's own words, the requirements met, partly met and open with the profile entry
//!    behind them, the points to check, Schwerpunkte, target role and wishes,
//! 5. the method (the skill's rules for requirements and the five frame rows),
//! 6. the one scoring rubric of the app and the skill (`ai_rubric.de.md`),
//! 7. a fixed answer format for a consultant who decides whether to apply.
//!
//! Their text is content for the assistant, not interface prose, in the app's language: the
//! German words ([`de`]) are an external contract - do not translate; [`en`] says the same in
//! English with the English rubric (`ai_rubric.en.md`, the same bands and caps,
//! `core/tests/rubric.rs`). The profile keys stay German in both: they are the profile's own.
//! No engine code reaches the text: every reason is said in words (an exhaustive `match` in
//! both languages).

mod de;
mod en;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use jiff::civil::Date;
use serde_json::{Map, Value};

use crate::error::Result;
use crate::matching::{
    Assessment, CriterionInfo, CriterionKey, CriterionState, CriterionStatus, Reason, ReasonCode,
    ReasonKind, Verdict, Via, Weight,
};
use crate::model::{Band, band};
use crate::pipeline::LocalMatcher;
use crate::portal::Facts;
use crate::settings::Language;
use crate::store::{JobRow, Store};
use crate::text::{one_line, truncate_chars};
use crate::view::{DetailState, JobView, WorkMode};

/// Most characters of the profile in the prompt (a longer one is cut, marked as cut).
pub const MAX_PROFILE_CHARS: usize = 8_000;
/// Most characters of the ad text in the prompt (the skill reads as much).
pub const MAX_AD_CHARS: usize = 12_000;
/// Most characters of each ad text in the comparison of the best matches.
pub const MAX_TOP_AD_CHARS: usize = 6_000;
/// How many jobs the comparison takes (fewer or more are brought into this range).
pub const TOP_LIMITS: RangeInclusive<usize> = 3..=5;
/// Longest passage of the ad the pre-assessment quotes.
const MAX_PASSAGE_CHARS: usize = 160;
/// Places of a permanent role the pre-assessment names before "and n more".
const MAX_PLACES: usize = 4;

/// One job of a prompt.
#[derive(Debug, Clone, Copy)]
pub struct PromptJob<'a> {
    pub job: &'a JobView,
    pub url: &'a str,
    /// The stored text: the full ad, or the teaser (`job.detail` says which).
    pub text: Option<&'a str>,
    /// What the portal page itself states (rate, start, level ...), in its words.
    pub page: Option<&'a Facts>,
    /// The app's assessment, made afresh from the stored text; `None` without a usable
    /// profile.
    pub assessment: Option<&'a Assessment>,
}

/// What a prompt takes of one stored job, owned: the reader's row, the link, the stored text,
/// the page's facts and a fresh assessment (reasons are not stored).
#[derive(Debug, Clone)]
pub struct PromptSource {
    pub view: JobView,
    pub url: String,
    pub text: Option<String>,
    pub page: Option<Facts>,
    pub assessment: Option<Assessment>,
}

impl PromptSource {
    /// The job as the reader shows it, assessed afresh from its stored text by `matcher`
    /// (an engine panic leaves it without an assessment, like the reader).
    pub fn load(store: &Store, matcher: Option<&LocalMatcher>, row: &JobRow) -> Result<Self> {
        let text = store.description(&row.key)?;
        let assessment = matcher.filter(|m| m.usable()).and_then(|m| {
            crate::pipeline::score::guarded(&row.key, || m.assessment(row, text.as_deref()))
                .flatten()
        });
        Ok(PromptSource {
            view: JobView::from(row),
            url: row.url.to_string(),
            text,
            page: row.facts.clone(),
            assessment,
        })
    }

    pub fn job(&self) -> PromptJob<'_> {
        PromptJob {
            job: &self.view,
            url: &self.url,
            text: self.text.as_deref(),
            page: self.page.as_ref(),
            assessment: self.assessment.as_ref(),
        }
    }
}

/// The prompt for one job in the app's language (the ad text at most [`MAX_AD_CHARS`]).
pub fn ai_prompt(profile: &Value, item: PromptJob<'_>, language: Language) -> String {
    let w = wording(language);
    let t = w.words();
    let consultant = Consultant::new(profile);
    let ad = ad_parts(w, item, MAX_AD_CHARS, false);
    let blocks = [
        section(1, t.task_heading, t.intro),
        profile_block(w, profile),
        section(1, t.ad_heading, ""),
        section(2, t.facts_heading, &ad.facts),
        section(2, t.text_heading, &ad.text),
        section(
            1,
            t.pre_heading,
            &format!("{}\n\n{}", t.pre_note, pre_assessment(w, item, &consultant)),
        ),
        section(1, t.method_heading, t.method),
        t.rubric.trim_end().to_owned(),
        section(1, t.answer_heading, t.answer),
    ];
    finish(&blocks)
}

/// One prompt that compares the best current matches (the favourites first) in the app's
/// language: every job with its key facts, its text (at most [`MAX_TOP_AD_CHARS`]; the prompt
/// says when a text was cut) and the app's pre-assessment, then one method, the rubric and an
/// answer format with a ranking first.
pub fn ai_prompt_top(profile: &Value, jobs: &[PromptJob<'_>], language: Language) -> String {
    let w = wording(language);
    let t = w.words();
    let consultant = Consultant::new(profile);
    let mut blocks = vec![
        section(1, t.task_heading, t.top_intro),
        profile_block(w, profile),
    ];
    let mut job_blocks = Vec::with_capacity(jobs.len());
    for (n, item) in jobs.iter().enumerate() {
        let ad = ad_parts(w, *item, MAX_TOP_AD_CHARS, true);
        job_blocks.push(
            [
                format!("## {}", w.job_heading(n + 1, title_of(item.job, t))),
                section(3, t.facts_heading, &ad.facts),
                section(3, t.text_heading, &ad.text),
                section(3, t.pre_heading, &pre_assessment(w, *item, &consultant)),
            ]
            .join("\n\n"),
        );
    }
    let pinned = jobs.iter().filter(|j| j.job.pinned).count();
    let note = format!("{} {}", w.top_note(jobs.len(), pinned), t.facts_note);
    blocks.push(section(1, t.jobs_heading, &note));
    blocks.extend(job_blocks);
    blocks.push(section(1, t.method_heading, t.method));
    blocks.push(t.rubric.trim_end().to_owned());
    blocks.push(section(1, t.answer_heading, t.top_answer));
    finish(&blocks)
}

/// The prompt's blocks, one blank line apart, with a final line break.
fn finish(blocks: &[String]) -> String {
    let mut out = blocks
        .iter()
        .map(|b| b.trim_end())
        .filter(|b| !b.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    out.push('\n');
    out
}

/// A heading of `level` and its body (none when empty).
fn section(level: usize, heading: &str, body: &str) -> String {
    let marks = "#".repeat(level);
    if body.trim().is_empty() {
        format!("{marks} {heading}")
    } else {
        format!("{marks} {heading}\n\n{}", body.trim_end())
    }
}

// ------------------------------------------------------------------------- the words

/// The fixed texts of the prompts in one language.
struct Words {
    rubric: &'static str,
    task_heading: &'static str,
    intro: &'static str,
    top_intro: &'static str,
    profile_heading: &'static str,
    profile_note: &'static str,
    glossary_intro: &'static str,
    /// Profile keys and what they mean; only the keys the profile holds are listed.
    glossary: &'static [(&'static str, &'static str)],
    ad_heading: &'static str,
    facts_heading: &'static str,
    facts_note: &'static str,
    text_heading: &'static str,
    text_full: &'static str,
    text_teaser: &'static str,
    text_short: &'static str,
    text_none: &'static str,
    pre_heading: &'static str,
    pre_note: &'static str,
    no_assessment: &'static str,
    /// The user marked an excluded job as fitting anyway.
    overridden: &'static str,
    method_heading: &'static str,
    method: &'static str,
    answer_heading: &'static str,
    answer: &'static str,
    top_answer: &'static str,
    jobs_heading: &'static str,
    /// What marks a cut profile or ad.
    cut: &'static str,
    untitled: &'static str,
    /// A key fact the app did not find.
    unknown: &'static str,
    labels: Labels,
}

/// Labels of the key facts and of the groups of the pre-assessment.
struct Labels {
    title: &'static str,
    company: &'static str,
    location: &'static str,
    portal: &'static str,
    contract: &'static str,
    pay: &'static str,
    start: &'static str,
    duration: &'static str,
    remote: &'static str,
    /// The page's employment type ("Befristet", "Freiberuflich").
    employment: &'static str,
    level: &'static str,
    function: &'static str,
    industries: &'static str,
    skills: &'static str,
    mail: &'static str,
    pinned: &'static str,
    status: &'static str,
    link: &'static str,
    /// The page says the ad takes no more applications.
    closed: &'static str,
    /// The ad is gone from the portal.
    gone: &'static str,
    /// Per the portal page ("laut Portalseite").
    per_page: &'static str,
    /// Per the location field.
    per_location: &'static str,
    exclusion: &'static str,
    criteria: &'static str,
    met: &'static str,
    partial: &'static str,
    open: &'static str,
    checks: &'static str,
    preferences: &'static str,
}

/// A requirement of the pre-assessment, in neutral terms.
struct Requirement<'a> {
    /// The ad's words (or a term the app found in the text).
    quote: &'a str,
    /// A term of the text, no stated requirement.
    term: bool,
    nice: bool,
    /// `skill`, `language`, `degree`, `licence`, `soft` or `frame`.
    class: &'a str,
    /// Years the requirement asks for.
    years: Option<i64>,
    evidence: Option<Entry>,
    /// The Schwerpunkt it meets in full (it counts double).
    focus: Option<&'a str>,
}

/// The profile entry behind a requirement.
struct Entry {
    text: String,
    years: Option<i64>,
    via: Via,
}

/// Everything a language says in its own words.
trait Wording: Sync {
    fn words(&self) -> &'static Words;
    fn quote(&self, text: &str) -> String;
    fn date(&self, day: Date) -> String;
    fn rate(&self, rate: i64, hourly: bool, currency: Option<&str>) -> String;
    fn rate_open(&self) -> &'static str;
    fn salary(&self, amount: i64, currency: Option<&str>, lower_bound: bool) -> String;
    /// `interim`, `permanent`, `anue` or `unclear`.
    fn contract(&self, kind: &str, inferred: bool) -> String;
    /// `now`, `vague` or an ISO date.
    fn start(&self, code: &str) -> String;
    fn months(&self, months: u16) -> String;
    fn remote(&self, from: u8, to: u8) -> String;
    fn work_mode(&self, mode: WorkMode) -> &'static str;
    fn job_heading(&self, n: usize, title: &str) -> String;
    fn top_note(&self, jobs: usize, pinned: usize) -> String;
    fn text_cut(&self, max: usize) -> String;
    fn scored(&self, score: u8, band: Band) -> String;
    fn excluded(&self, score: u8) -> String;
    fn unscorable(&self) -> &'static str;
    fn musts(&self, met: u16, partial: u16, open: u16, total: u16) -> String;
    fn nices(&self, met: u16, total: u16) -> String;
    /// The criterion with the profile's threshold (`profile`: the params the engine read).
    fn criterion(&self, key: CriterionKey, profile: &Map<String, Value>) -> String;
    fn criterion_status(&self, status: CriterionStatus) -> &'static str;
    /// The ad's value of a criterion (`ad`: the params of its state), if it names one.
    fn criterion_value(&self, key: CriterionKey, ad: &Map<String, Value>) -> Option<String>;
    /// A reason in one sentence; `None` for requirements and terms (they are quoted).
    fn reason(&self, code: ReasonCode, params: &Map<String, Value>) -> Option<String>;
    fn requirement(&self, line: &Requirement<'_>) -> String;
}

fn wording(language: Language) -> &'static dyn Wording {
    match language {
        Language::De => &de::German,
        Language::En => &en::English,
    }
}

// ------------------------------------------------------------------------- the profile

/// The consultant as the prompt reads the profile: its JSON and the thresholds of its hard
/// criteria as the engine reads them.
struct Consultant<'a> {
    profile: &'a Value,
    /// The hard criteria as the engine reads them (thresholds).
    criteria: Vec<CriterionInfo>,
}

impl<'a> Consultant<'a> {
    fn new(profile: &'a Value) -> Self {
        Consultant {
            profile,
            criteria: crate::matching::profile_criteria(profile),
        }
    }

    fn threshold(&self, key: CriterionKey) -> Map<String, Value> {
        self.criteria
            .iter()
            .find(|c| c.key == key)
            .map(|c| c.params.clone())
            .unwrap_or_default()
    }
}

/// The profile block: a note, the glossary of the keys it holds, the JSON without personal
/// data.
fn profile_block(w: &dyn Wording, profile: &Value) -> String {
    let t = w.words();
    let clean = super::personal::scrub_profile(profile);
    let mut keys = BTreeSet::new();
    collect_keys(&clean, &mut keys);
    let glossary: Vec<String> = t
        .glossary
        .iter()
        .filter(|(key, _)| keys.contains(*key))
        .map(|(key, meaning)| format!("- `{key}`: {meaning}"))
        .collect();
    let mut body = t.profile_note.to_owned();
    if !glossary.is_empty() {
        body = format!("{body} {}\n\n{}", t.glossary_intro, glossary.join("\n"));
    }
    let json = profile_json(&clean, t.cut);
    section(
        1,
        t.profile_heading,
        &format!("{body}\n\n```json\n{json}\n```"),
    )
}

fn collect_keys(value: &Value, keys: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                keys.insert(key.clone());
                collect_keys(inner, keys);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_keys(v, keys)),
        _ => {}
    }
}

/// The profile as JSON, at most [`MAX_PROFILE_CHARS`] long (pretty when it fits, compact when
/// that fits, else cut and marked with `mark`).
fn profile_json(clean: &Value, mark: &str) -> String {
    let pretty = serde_json::to_string_pretty(clean).unwrap_or_default();
    if pretty.chars().count() <= MAX_PROFILE_CHARS {
        return pretty;
    }
    let compact = serde_json::to_string(clean).unwrap_or_default();
    cut(&compact, MAX_PROFILE_CHARS, mark)
}

/// The profile entry at a JSON path of the engine (`kernkompetenzen[3].kompetenz`,
/// `kernkompetenzen[0].auch[1]`) with the years of the nearest object that states them.
fn entry_years(profile: &Value, path: &str) -> Option<i64> {
    let mut trail = vec![profile];
    let mut node = profile;
    for part in path.split('.') {
        let (name, indices) = match part.find('[') {
            Some(at) => (&part[..at], &part[at..]),
            None => (part, ""),
        };
        if !name.is_empty() {
            node = node.get(name)?;
            trail.push(node);
        }
        for index in indices
            .split(['[', ']'])
            .filter(|s| !s.is_empty())
            .map(str::parse::<usize>)
        {
            node = node.get(index.ok()?)?;
            trail.push(node);
        }
    }
    trail
        .iter()
        .rev()
        .filter_map(|v| v.as_object())
        .find_map(|o| o.get("jahre").or_else(|| o.get("years")).and_then(as_int))
}

// ------------------------------------------------------------------------- the ad

/// The key facts and the text block of an ad.
struct AdParts {
    facts: String,
    text: String,
}

fn ad_parts(w: &dyn Wording, item: PromptJob<'_>, max: usize, top: bool) -> AdParts {
    let t = w.words();
    let text = item.text.map(str::trim).filter(|s| !s.is_empty());
    let cut_now = text.is_some_and(|s| s.chars().count() > max);
    let status = match (text, item.job.detail) {
        (None, _) => t.text_none,
        (Some(_), DetailState::Teaser) => t.text_teaser,
        (Some(_), _) if item.job.short => t.text_short,
        (Some(_), _) => t.text_full,
    };
    let body = match text {
        Some(text) => {
            let shown = cut(text, max, t.cut);
            let fence = fence_for(&shown);
            let note = if cut_now {
                format!(" {}", w.text_cut(max))
            } else {
                String::new()
            };
            format!("{status}{note}\n\n{fence}text\n{shown}\n{fence}")
        }
        None => status.to_owned(),
    };
    AdParts {
        // The comparison says it once for all jobs.
        facts: if top {
            key_facts(w, item, top)
        } else {
            format!("{}\n\n{}", t.facts_note, key_facts(w, item, top))
        },
        text: body,
    }
}

/// A code fence longer than any run of backticks in `text`.
fn fence_for(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

/// The key facts: the app's reading (the page facts first, then the text), the page's own
/// words where the app read nothing, and "not found" for what neither gave.
fn key_facts(w: &dyn Wording, item: PromptJob<'_>, top: bool) -> String {
    let t = w.words();
    let l = &t.labels;
    let job = item.job;
    let [contract, pay, start, duration, remote] =
        terms(w, item).map(|value| value.unwrap_or_else(|| t.unknown.to_owned()));
    let mut lines = vec![
        line(l.title, title_of(job, t)),
        line(l.company, &or_value(&job.company, t.unknown)),
        line(l.location, &or_value(&job.location, t.unknown)),
        line(l.portal, job.portal.label()),
        line(l.contract, &contract),
        line(l.pay, &pay),
        line(l.start, &start),
        line(l.duration, &duration),
        line(l.remote, &remote),
    ];
    // The page's own labels, always in its words (LinkedIn's "Befristet" may mean a
    // fixed-term employment as well as a project).
    if let Some(page) = item.page {
        let skills = (!page.skills.is_empty()).then(|| page.skills.join(", "));
        for (label, value) in [
            (l.employment, page.employment_type.as_ref()),
            (l.level, page.level.as_ref()),
            (l.function, page.function.as_ref()),
            (l.industries, page.industries.as_ref()),
            (l.skills, skills.as_ref()),
        ] {
            if let Some(value) = page_words(w, value) {
                lines.push(line(label, &value));
            }
        }
    }
    if let Some(day) = job.mail_date {
        lines.push(line(l.mail, &w.date(crate::time::local_date(day))));
    }
    if top && job.pinned {
        lines.push(format!("- {}", l.pinned));
    }
    if job.closed {
        lines.push(line(l.status, l.closed));
    } else if job.detail == DetailState::Gone {
        lines.push(line(l.status, l.gone));
    }
    if !item.url.trim().is_empty() {
        lines.push(line(l.link, item.url.trim()));
    }
    lines.join("\n")
}

/// Contract type, pay, start, duration and remote share as the app read them, else in the
/// page's words (the location's work mode for the remote share); `None` where nothing says.
fn terms(w: &dyn Wording, item: PromptJob<'_>) -> [Option<String>; 5] {
    let page = item.page;
    let facts = item.assessment.map(|a| &a.facts);
    let contract = item
        .assessment
        .and_then(|a| {
            a.reasons
                .iter()
                .find(|r| r.code == ReasonCode::ContractType)
        })
        .and_then(|r| {
            text_param(&r.params, "type").map(|kind| w.contract(kind, flag(&r.params, "inferred")))
        })
        .or_else(|| {
            facts
                .and_then(|f| f.contract.as_deref())
                .map(|kind| w.contract(kind, false))
        });
    let pay = facts
        .and_then(|f| {
            f.rate.map(|rate| {
                w.rate(
                    i64::from(rate),
                    f.hourly.unwrap_or(false),
                    f.currency.as_deref(),
                )
            })
        })
        .or_else(|| {
            item.assessment
                .and_then(salary_of)
                .map(|(amount, currency, lower)| w.salary(amount, currency.as_deref(), lower))
        })
        .or_else(|| {
            facts
                .and_then(|f| f.rate_open)
                .filter(|open| *open)
                .map(|_| w.rate_open().to_owned())
        })
        .or_else(|| page_words(w, page.and_then(|p| p.rate.as_ref())));
    let start = facts
        .and_then(|f| f.start.as_deref())
        .map(|code| w.start(code))
        .or_else(|| page_words(w, page.and_then(|p| p.start.as_ref())));
    let duration = facts
        .and_then(|f| f.months)
        .map(|m| w.months(m))
        .or_else(|| page_words(w, page.and_then(|p| p.duration.as_ref())));
    let remote = facts
        .and_then(|f| Some(w.remote(f.remote_from?, f.remote_to.or(f.remote_from)?)))
        .or_else(|| {
            page.and_then(|p| p.remote_percent)
                .map(|share| w.remote(share, share))
        })
        .or_else(|| page_words(w, page.and_then(|p| p.remote.as_ref())))
        .or_else(|| {
            item.job
                .work_mode
                .map(|mode| format!("{} ({})", w.work_mode(mode), w.words().labels.per_location))
        });
    [contract, pay, start, duration, remote]
}

/// A fact in the page's own words, quoted and marked as the page's.
fn page_words(w: &dyn Wording, value: Option<&String>) -> Option<String> {
    let words = one_line(value?);
    (!words.is_empty()).then(|| format!("{} ({})", w.quote(&words), w.words().labels.per_page))
}

fn line(label: &str, value: &str) -> String {
    format!("- {label}: {value}")
}

fn or_value(value: &str, unknown: &str) -> String {
    if value.trim().is_empty() {
        unknown.to_owned()
    } else {
        value.trim().to_owned()
    }
}

fn title_of<'a>(job: &'a JobView, t: &'static Words) -> &'a str {
    if job.title.trim().is_empty() {
        t.untitled
    } else {
        job.title.trim()
    }
}

/// The stated annual salary: amount, currency (not EUR), stated as a lower bound.
fn salary_of(a: &Assessment) -> Option<(i64, Option<String>, bool)> {
    let from_reason = a
        .reasons
        .iter()
        .find(|r| r.code == ReasonCode::Salary)
        .and_then(|r| {
            let amount = int(&r.params, "salary")?;
            let currency = text_param(&r.params, "currency")
                .filter(|c| *c != "EUR")
                .map(str::to_owned);
            Some((amount, currency, flag(&r.params, "lowerBound")))
        });
    from_reason.or_else(|| {
        a.criteria
            .iter()
            .find(|c| c.key == CriterionKey::MinSalary)
            .and_then(|c| int(&c.params, "salary"))
            .map(|amount| (amount, None, false))
    })
}

// ------------------------------------------------------------------------- pre-assessment

/// The app's pre-assessment of one job: the result, the counts, the hard criteria, the
/// requirements by status, the points to check and the Schwerpunkte, target role and wishes.
fn pre_assessment(w: &dyn Wording, item: PromptJob<'_>, consultant: &Consultant<'_>) -> String {
    let t = w.words();
    let l = &t.labels;
    let Some(assessment) = item.assessment else {
        return t.no_assessment.to_owned();
    };
    let text = item.text.unwrap_or_default();
    let quoted = |reason: &Reason| reason_passage(assessment, reason, text).map(|p| w.quote(&p));
    let mut blocks = vec![result_lines(w, item, assessment)];

    // The hard criteria the profile sets, with the reason that decided each.
    let mut linked = BTreeSet::new();
    let criteria: Vec<String> = assessment
        .criteria
        .iter()
        .filter(|c| c.status != CriterionStatus::Inactive)
        .map(|c| {
            let reason = c
                .reason
                .and_then(|id| assessment.reasons.iter().find(|r| r.id == id));
            if let Some(reason) = reason {
                linked.insert(reason.id);
            }
            criterion_line(w, consultant, assessment, c, reason, text)
        })
        .collect();
    push_group(&mut blocks, l.criteria, &criteria);

    // The requirements by status (weighted ones only: the rest does not count).
    for (kind, label) in [
        (ReasonKind::Met, l.met),
        (ReasonKind::Partial, l.partial),
        (ReasonKind::Open, l.open),
    ] {
        let lines: Vec<String> = assessment
            .reasons
            .iter()
            .filter(|r| {
                r.kind == kind
                    && matches!(r.code, ReasonCode::Requirement | ReasonCode::Term)
                    && matches!(r.weight, Weight::Must | Weight::Nice)
            })
            .map(|r| format!("- {}", w.requirement(&requirement(r, consultant.profile))))
            .collect();
        push_group(&mut blocks, label, &lines);
    }

    // Points to check that no criterion above and no result carries already.
    let checks: Vec<String> = assessment
        .reasons
        .iter()
        .filter(|r| r.kind == ReasonKind::Check && !linked.contains(&r.id))
        .filter(|r| !(r.code == ReasonCode::ShortText && assessment.verdict == Verdict::Unscorable))
        .filter_map(|r| {
            w.reason(r.code, &r.params)
                .map(|said| format!("- {}", sentence(Some(said), quoted(r))))
        })
        .collect();
    push_group(&mut blocks, l.checks, &checks);

    // A Schwerpunkt's words stand at its requirements already.
    let preferences: Vec<String> = assessment
        .reasons
        .iter()
        .filter(|r| is_preference(r.code))
        .filter_map(|r| {
            let quote = if r.code == ReasonCode::Focus {
                None
            } else {
                quoted(r)
            };
            w.reason(r.code, &r.params)
                .map(|said| format!("- {}", sentence(Some(said), quote)))
        })
        .collect();
    push_group(&mut blocks, l.preferences, &preferences);
    blocks.join("\n\n")
}

/// The result: the score and band, or the exclusion with each reason (and the user's "fits
/// anyway"), or no score; then the counts of musts and nice-to-haves.
fn result_lines(w: &dyn Wording, item: PromptJob<'_>, assessment: &Assessment) -> String {
    let t = w.words();
    let text = item.text.unwrap_or_default();
    let mut lines = Vec::new();
    match assessment.verdict {
        Verdict::Excluded => {
            lines.push(format!("- {}", w.excluded(assessment.score)));
            for reason in assessment
                .reasons
                .iter()
                .filter(|r| r.kind == ReasonKind::Violation)
            {
                let quote = reason_passage(assessment, reason, text).map(|p| w.quote(&p));
                lines.push(format!(
                    "- {}: {}",
                    t.labels.exclusion,
                    sentence(w.reason(reason.code, &reason.params), quote)
                ));
            }
            if item.job.overridden {
                lines.push(format!("- {}", t.overridden));
            }
        }
        Verdict::Unscorable => lines.push(format!("- {}", w.unscorable())),
        Verdict::Scored => lines.push(format!(
            "- {}",
            w.scored(assessment.score, band(assessment.score))
        )),
    }
    let counts = &assessment.summary;
    lines.push(format!(
        "- {}",
        w.musts(
            counts.must_met,
            counts.must_partial,
            counts.must_open,
            counts.must_total
        )
    ));
    if counts.nice_total > 0 {
        lines.push(format!("- {}", w.nices(counts.nice_met, counts.nice_total)));
    }
    lines.join("\n")
}

fn push_group(blocks: &mut Vec<String>, label: &str, lines: &[String]) {
    if !lines.is_empty() {
        blocks.push(format!("**{label}**\n{}", lines.join("\n")));
    }
}

fn is_preference(code: ReasonCode) -> bool {
    matches!(
        code,
        ReasonCode::Focus
            | ReasonCode::TargetRole
            | ReasonCode::DayRateWish
            | ReasonCode::RemoteWish
            | ReasonCode::RegionWish
            | ReasonCode::IndustryWish
    )
}

/// A said reason with the ad's words behind it.
fn sentence(said: Option<String>, quote: Option<String>) -> String {
    match (said, quote) {
        (Some(said), Some(quote)) => format!("{said} {quote}"),
        (Some(said), None) => said,
        (None, Some(quote)) => quote,
        (None, None) => String::new(),
    }
}

/// One hard criterion: the profile's threshold, the state, the reason that decided it and
/// the ad's words (or its value).
fn criterion_line(
    w: &dyn Wording,
    consultant: &Consultant<'_>,
    a: &Assessment,
    state: &CriterionState,
    reason: Option<&Reason>,
    text: &str,
) -> String {
    let label = w.criterion(state.key, &consultant.threshold(state.key));
    let status = w.criterion_status(state.status);
    let passage = reason
        .and_then(|r| reason_passage(a, r, text))
        .or_else(|| state.range.and_then(|range| passage(text, range)))
        .map(|p| w.quote(&p));
    // A violation is said in the result above; a check only here.
    let said = reason
        .filter(|r| r.kind == ReasonKind::Check)
        .and_then(|r| w.reason(r.code, &r.params));
    let value = w.criterion_value(state.key, &state.params);
    match (said, value, passage) {
        (Some(said), _, passage) => {
            let tail = passage.map(|p| format!(" {p}")).unwrap_or_default();
            format!("- {label}: {status}. {said}{tail}")
        }
        (None, Some(value), Some(passage)) => {
            format!("- {label}: {status}, {value} ({passage})")
        }
        (None, Some(value), None) => format!("- {label}: {status}, {value}"),
        (None, None, Some(passage)) => format!("- {label}: {status}, {passage}"),
        (None, None, None) => format!("- {label}: {status}"),
    }
}

/// A requirement reason in neutral terms, its profile entry with the years the profile states.
fn requirement<'a>(reason: &'a Reason, profile: &Value) -> Requirement<'a> {
    let evidence = reason.evidence.as_ref().map(|e| Entry {
        text: super::personal::scrub_text(&e.profile),
        years: entry_years(profile, &e.path),
        via: e.via,
    });
    Requirement {
        quote: reason.label.as_deref().unwrap_or_default(),
        term: reason.code == ReasonCode::Term,
        nice: reason.weight == Weight::Nice,
        class: text_param(&reason.params, "class").unwrap_or("skill"),
        years: int(&reason.params, "years"),
        evidence,
        focus: text_param(&reason.params, "focus"),
    }
}

/// The ad's words a reason points at (its first highlighted passage).
fn reason_passage(a: &Assessment, reason: &Reason, text: &str) -> Option<String> {
    reason.ranges.iter().find_map(|id| {
        a.highlights
            .iter()
            .find(|h| h.id == *id)
            .and_then(|h| passage(text, (h.start, h.end)))
    })
}

/// The ad's words between two UTF-16 offsets, on one line and at most
/// [`MAX_PASSAGE_CHARS`] long.
fn passage(text: &str, (start, end): (u32, u32)) -> Option<String> {
    if end <= start {
        return None;
    }
    let mut units = 0u32;
    let mut from = None;
    let mut to = text.len();
    for (at, c) in text.char_indices() {
        if from.is_none() && units >= start {
            from = Some(at);
        }
        if units >= end {
            to = at;
            break;
        }
        units += u32::try_from(c.len_utf16()).unwrap_or(1);
    }
    // Without a list mark in front: the words, not the layout.
    let words = one_line(text.get(from?..to)?);
    let words = words
        .trim_start_matches(['-', '*', '•', '·', '–', ' '])
        .to_owned();
    if words.is_empty() {
        return None;
    }
    if words.chars().count() <= MAX_PASSAGE_CHARS {
        Some(words)
    } else {
        Some(format!(
            "{}…",
            truncate_chars(&words, MAX_PASSAGE_CHARS - 1).trim_end()
        ))
    }
}

// ------------------------------------------------------------------------- helpers

fn cut(text: &str, max: usize, mark: &str) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!("{} {mark}", truncate_chars(text, max))
    }
}

/// A whole number, also written as text (`"1000"`).
fn as_int(value: &Value) -> Option<i64> {
    match value {
        // A whole number written as 1100.0 counts too.
        Value::Number(n) => n
            .as_i64()
            .or_else(|| format!("{:.0}", n.as_f64()?).parse().ok()),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn int(params: &Map<String, Value>, key: &str) -> Option<i64> {
    params.get(key).and_then(as_int)
}

fn text_param<'a>(params: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn flag(params: &Map<String, Value>, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// A list param as texts (`["DE", "AT"]` or `"DE, AT"`).
fn list_param(params: &Map<String, Value>, key: &str) -> Vec<String> {
    match params.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Digits grouped by threes with `separator` (`1450` -> `1.450`).
fn grouped(n: i64, separator: char) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(separator);
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// The first `MAX_PLACES` places and how many more there are.
fn some_places(places: &[String]) -> (String, usize) {
    let shown = places.iter().take(MAX_PLACES).cloned().collect::<Vec<_>>();
    (shown.join(", "), places.len().saturating_sub(MAX_PLACES))
}
