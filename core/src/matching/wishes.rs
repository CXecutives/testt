//! Wishes of `einsatzpraeferenzen` (`ENGINE_VERSION` 4): day rate, remote share, regions and
//! industries. Each wish is off while its key is missing; a wish never excludes (exclusions
//! stay in `harte_kriterien`).
//!
//! Every wish ends in a state: met adds its points, clearly missed takes them, near and
//! unknown change nothing. The points are `WISH_RATE`, `WISH_REMOTE`, `WISH_REGION` and
//! `WISH_INDUSTRY` per-mille; their sum is bounded by `WISH_MAX`, so the wishes never
//! outweigh the fit.
//!
//! - Day rate (`tagessatz_wunsch`, not for permanent roles): the upper bound of the stated
//!   EUR rate per day (hourly x 8) at or above the wish is met, at least `WISH_RATE_NEAR`
//!   of it near, below that missed; no rate or another currency is unknown.
//! - Remote (`remote`: `voll`, `ueberwiegend`, `teilweise`, `vor_ort`, or old free text such
//!   as `mindestens 50 %`) against the ad's remote share (facts, `N % remote`, office or
//!   remote days, full-remote wording, hybrid wording, on site only): a minimum `m` is met
//!   when the share is at least `m`, missed when the ad is on site only or its share stays
//!   more than `REMOTE_MARGIN` below `m`; `vor_ort` is met up to `ONSITE_MAX` percent.
//! - Regions (`regionen`: places, German states, regions such as `Rhein-Main`, countries):
//!   a fully remote ad is met; a wished place in the location, a location line or an
//!   on-site sentence is met; another known place (larger German cities, states, foreign
//!   cities and countries) is missed, or near with at least `REGION_REMOTE_NEAR` percent
//!   remote; no known place (a country of the profile only, a small town) is unknown.
//! - Industries (`branchen`) against the title, the company (LinkedIn only: on the
//!   freelance portals it is the posting agency) and the ad's text outside requirements
//!   and tasks: a wished industry named is met, only other industries named is missed,
//!   none named is unknown.

use std::ops::Range;
use std::sync::LazyLock;

use serde_json::{Value, json};

use super::atoms::{self, Fit, Vocab, fold};
use super::contract::ContractKind;
use super::facts::{self, JobFacts, Segment, fact};
use super::focus::texts;
use super::job::contains_word;
use super::lexicon::{self, engine as core_lex, wishes as lex};
use super::params::{
    HYBRID_SHARE, ONSITE_MAX, REGION_REMOTE_NEAR, REMOTE_FULL, REMOTE_MARGIN, REMOTE_MOSTLY,
    REMOTE_NAMED_SHARE, REMOTE_PARTLY, WISH_INDUSTRY, WISH_RATE, WISH_RATE_NEAR, WISH_REGION,
    WISH_REMOTE, WORKDAYS,
};
use super::permanent::percents;
use super::types::{ReasonCode, ReasonKind, WishInfo, WishKey};
use crate::portal::Portal;

/// The remote share a profile wishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteWish {
    /// At least this share (percent).
    Min(u64),
    /// Presence on site.
    OnSite,
}

/// An industry wish: its text, the industries it names and its atoms (for industries the
/// word list does not know).
#[derive(Debug, Clone)]
struct Industry {
    text: String,
    keys: Vec<&'static str>,
    atoms: Vec<String>,
}

/// The wishes of a profile.
#[derive(Debug, Clone, Default)]
pub(crate) struct Wishes {
    pub rate: Option<u64>,
    pub remote: Option<RemoteWish>,
    /// Regions as written.
    pub regions: Option<Vec<String>>,
    /// Industries as written.
    pub industries: Option<Vec<String>>,
    /// The regions folded, with the places of states, regions and countries.
    places: Vec<String>,
    industry: Vec<Industry>,
}

/// The wishes section: `einsatzpraeferenzen` (or `preferences`).
fn section(data: &Value) -> Option<&Value> {
    lexicon::KEY_PREFERENCES_ALIASES
        .iter()
        .find_map(|key| data.get(*key).filter(|v| v.is_object()))
}

fn entry<'a>(section: &'a Value, keys: &[&'static str]) -> Option<(&'static str, &'a Value)> {
    keys.iter()
        .find_map(|k| section.get(*k).filter(|v| !v.is_null()).map(|v| (*k, v)))
}

/// The remote wish of a value: a level word or a percentage (`mindestens 50 %`).
pub(crate) fn remote_wish(value: &Value) -> Option<RemoteWish> {
    let share = |p: u64| {
        if p == 0 {
            RemoteWish::OnSite
        } else {
            RemoteWish::Min(p.min(100))
        }
    };
    if let Some(p) = value.as_u64() {
        return Some(share(p));
    }
    let folded = fold(value.as_str()?).replace(['_', '-'], " ");
    if let Some(&p) = percents(&folded).first() {
        return Some(share(p));
    }
    let level = lex::REMOTE_WISH_WORDS
        .iter()
        .find(|(word, _)| contains_word(&folded, word))
        .map(|&(_, level)| level)?;
    Some(match level {
        "onsite" => RemoteWish::OnSite,
        "full" => RemoteWish::Min(REMOTE_FULL),
        "mostly" => RemoteWish::Min(REMOTE_MOSTLY),
        _ => RemoteWish::Min(REMOTE_PARTLY),
    })
}

/// Every place a known region, state or country stands for, and all known places.
static KNOWN_PLACES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut places: Vec<&'static str> = core_lex::GERMAN_CITIES
        .iter()
        .copied()
        .chain(lex::REGION_PLACES.iter().map(|(region, _)| *region))
        .chain(
            lex::REGION_PLACES
                .iter()
                .flat_map(|(_, p)| p.iter().copied()),
        )
        .chain(core_lex::CITIES.iter().map(|(city, _)| *city))
        .chain(
            core_lex::COUNTRIES
                .iter()
                .filter(|(_, code)| *code != "DE")
                .map(|(name, _)| *name),
        )
        .collect();
    places.sort_unstable();
    places.dedup();
    places
});

/// German places (cities and states) for a region `Deutschland`.
fn german_places() -> impl Iterator<Item = &'static str> {
    core_lex::GERMAN_CITIES
        .iter()
        .copied()
        .chain(lex::REGION_PLACES.iter().map(|(region, _)| *region))
        .chain(
            lex::REGION_PLACES
                .iter()
                .flat_map(|(_, p)| p.iter().copied()),
        )
}

/// A folded region with every place it stands for.
fn expand(region: &str) -> Vec<String> {
    let mut places = vec![region.to_owned()];
    for (name, inside) in lex::REGION_PLACES {
        if *name == region {
            places.extend(inside.iter().map(|p| (*p).to_owned()));
        }
    }
    if let Some(&(_, code)) = core_lex::COUNTRIES.iter().find(|(name, _)| *name == region) {
        places.extend(
            core_lex::COUNTRIES
                .iter()
                .filter(|(_, c)| *c == code)
                .map(|(name, _)| (*name).to_owned()),
        );
        places.extend(
            core_lex::CITIES
                .iter()
                .filter(|(_, c)| *c == code)
                .map(|(city, _)| (*city).to_owned()),
        );
        if code == "DE" {
            places.extend(german_places().map(str::to_owned));
        }
    }
    places
}

/// Words of a text as written (letters, digits, `-`, `&`), without surrounding hyphens.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '&'))
        .map(|w| w.trim_matches('-'))
        .filter(|w| !w.is_empty())
}

/// Does a folded word stand for an entry of a word list: the same word, or a longer word
/// starting with an entry of at least six letters?
fn word_is(folded: &str, entry: &str) -> bool {
    folded == entry || (entry.len() >= 6 && folded.starts_with(entry))
}

/// Industries named in a text: (industry, the word as written).
fn industries_in(text: &str) -> Vec<(&'static str, String)> {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    for word in words(text) {
        let folded = fold(word);
        let whole = lex::INDUSTRY_WORDS
            .iter()
            .find(|(entry, _)| folded == *entry);
        let start = || {
            lex::INDUSTRIES
                .iter()
                .find(|(entry, _)| !entry.contains(' ') && word_is(&folded, entry))
        };
        if let Some(&(_, key)) = whole.or_else(start) {
            found.push((key, word.to_owned()));
        }
    }
    let folded = fold(text);
    for (phrase, key) in lex::INDUSTRIES.iter().filter(|(e, _)| e.contains(' ')) {
        if contains_word(&folded, phrase) {
            found.push((key, (*phrase).to_owned()));
        }
    }
    found
}

impl Wishes {
    /// Reads the wishes; keys present with a value that cannot be read are returned.
    pub(crate) fn new(data: &Value, vocab: &Vocab) -> (Self, Vec<(&'static str, String)>) {
        let mut wishes = Wishes::default();
        let mut unreadable = Vec::new();
        let Some(section) = section(data) else {
            return (wishes, unreadable);
        };
        if let Some((key, value)) = entry(section, lexicon::KEYS_RATE_WISH) {
            wishes.rate = facts::number(value).filter(|n| *n > 0);
            if wishes.rate.is_none() {
                unreadable.push((key, value.to_string()));
            }
        }
        if let Some((key, value)) = entry(section, lexicon::KEYS_REMOTE_WISH) {
            wishes.remote = remote_wish(value);
            if wishes.remote.is_none() {
                unreadable.push((key, value.to_string()));
            }
        }
        let mut list = |keys: &[&'static str]| {
            let (key, value) = entry(section, keys)?;
            let list = texts(value).filter(|l| !l.is_empty());
            if list.is_none() {
                unreadable.push((key, value.to_string()));
            }
            list
        };
        wishes.regions = list(lexicon::KEYS_REGIONS);
        wishes.industries = list(lexicon::KEYS_INDUSTRIES);
        let mut places: Vec<String> = wishes
            .regions
            .iter()
            .flatten()
            .flat_map(|r| expand(&fold(r)))
            .collect();
        places.sort();
        places.dedup();
        wishes.places = places;
        wishes.industry = wishes
            .industries
            .iter()
            .flatten()
            .map(|text| {
                let mut keys: Vec<&'static str> =
                    industries_in(text).into_iter().map(|(k, _)| k).collect();
                keys.sort_unstable();
                keys.dedup();
                Industry {
                    atoms: atoms::atoms(text, vocab),
                    keys,
                    text: text.clone(),
                }
            })
            .collect();
        (wishes, unreadable)
    }

    /// The wishes as understood (`set` and the values).
    pub(crate) fn info(&self) -> Vec<WishInfo> {
        let info = |key, set: bool, params: Value| WishInfo {
            key,
            set,
            params: params.as_object().cloned().unwrap_or_default(),
        };
        let remote = match self.remote {
            Some(RemoteWish::Min(min)) => json!({ "min": min, "onsite": false }),
            Some(RemoteWish::OnSite) => json!({ "min": null, "onsite": true }),
            None => json!({ "min": null, "onsite": null }),
        };
        vec![
            info(
                WishKey::DayRate,
                self.rate.is_some(),
                json!({ "wish": self.rate }),
            ),
            info(WishKey::Remote, self.remote.is_some(), remote),
            info(
                WishKey::Regions,
                self.regions.is_some(),
                json!({ "regions": self.regions }),
            ),
            info(
                WishKey::Industries,
                self.industries.is_some(),
                json!({ "industries": self.industries }),
            ),
        ]
    }

    /// The canonical view for the profile fingerprint.
    pub(crate) fn canonical(&self) -> String {
        let sorted = |list: &Option<Vec<String>>| {
            list.as_ref().map(|l| {
                let mut l: Vec<String> = l.iter().map(|t| fold(t)).collect();
                l.sort();
                l
            })
        };
        format!(
            "rate {:?} remote {:?} regions {:?} industries {:?}",
            self.rate,
            self.remote,
            sorted(&self.regions),
            sorted(&self.industries)
        )
    }

    fn any(&self) -> bool {
        self.rate.is_some()
            || self.remote.is_some()
            || self.regions.is_some()
            || self.industries.is_some()
    }
}

/// How a wish came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Met,
    Near,
    Missed,
    Unknown,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            Self::Met => "met",
            Self::Near => "near",
            Self::Missed => "missed",
            Self::Unknown => "unknown",
        }
    }

    /// The reader groups: met, met in part, open (missed or not stated).
    pub(crate) fn kind(self) -> ReasonKind {
        match self {
            Self::Met => ReasonKind::Met,
            Self::Near => ReasonKind::Partial,
            Self::Missed | Self::Unknown => ReasonKind::Open,
        }
    }

    fn points(self, weight: i64) -> i64 {
        match self {
            Self::Met => weight,
            Self::Missed => -weight,
            Self::Near | Self::Unknown => 0,
        }
    }
}

/// One wish for one job, with its points (per-mille) and the passages behind it.
#[derive(Debug, Clone)]
pub(crate) struct WishResult {
    pub code: ReasonCode,
    pub state: State,
    pub points: i64,
    pub params: Value,
    pub spans: Vec<Range<usize>>,
}

fn result(
    code: ReasonCode,
    state: State,
    weight: i64,
    mut params: Value,
    spans: Vec<Range<usize>>,
) -> WishResult {
    let points = state.points(weight);
    params["state"] = json!(state.name());
    // Points on the score scale (per-mille / 10).
    params["points"] = json!(points / 10);
    WishResult {
        code,
        state,
        points,
        params,
        spans,
    }
}

/// What an ad says for the wishes.
pub(crate) struct Ad<'a> {
    pub job: &'a JobFacts<'a>,
    pub company: &'a str,
    pub segments: &'a [Segment],
    pub folded: &'a str,
    pub contract: ContractKind,
    /// Lines outside requirements and tasks.
    pub context: &'a [Range<usize>],
    pub vocab: &'a Vocab,
}

/// The wishes for one ad, in the order day rate, remote, region, industry.
pub(crate) fn evaluate(wishes: &Wishes, ad: &Ad<'_>) -> Vec<WishResult> {
    if !wishes.any() {
        return Vec::new();
    }
    let share = remote_share(ad.job, ad.segments, ad.folded);
    let mut out = Vec::new();
    if let Some(wish) = wishes.rate
        && ad.contract != ContractKind::Permanent
    {
        out.push(rate(wish, ad));
    }
    if let Some(wish) = wishes.remote {
        out.push(remote(wish, share));
    }
    if wishes.regions.is_some() {
        out.push(region(&wishes.places, ad, share));
    }
    if wishes.industries.is_some() {
        out.push(industry(&wishes.industry, ad));
    }
    out
}

fn rate(wish: u64, ad: &Ad<'_>) -> WishResult {
    let code = ReasonCode::DayRateWish;
    let Some((rate, span)) = facts::stated_rate(ad.job, ad.segments) else {
        return result(
            code,
            State::Unknown,
            WISH_RATE,
            json!({ "wish": wish }),
            Vec::new(),
        );
    };
    let spans: Vec<Range<usize>> = span.into_iter().collect();
    if let Some(currency) = rate.currency {
        let params = json!({ "wish": wish, "currency": currency.to_uppercase() });
        return result(code, State::Unknown, WISH_RATE, params, spans);
    }
    let per_day = rate.per_day();
    let state = if per_day >= wish {
        State::Met
    } else if u128::from(per_day) * 1000 >= u128::from(wish) * u128::from(WISH_RATE_NEAR) {
        State::Near
    } else {
        State::Missed
    };
    let params = json!({ "wish": wish, "rate": per_day, "hourly": rate.hourly });
    result(code, state, WISH_RATE, params, spans)
}

/// Does a folded text name one of `list` (whole words, word starts from six letters on, or
/// phrases)?
fn names(folded: &str, list: &[&str]) -> bool {
    list.iter().any(|entry| {
        if entry.contains(' ') {
            contains_word(folded, entry)
        } else {
            words(folded).any(|w| word_is(w, entry))
        }
    })
}

fn has_remote(folded: &str) -> bool {
    names(folded, lex::REMOTE_AD_WORDS)
}

fn has_onsite(folded: &str) -> bool {
    names(folded, lex::ONSITE_AD_WORDS)
}

/// Shares one sentence states: `N % remote`, `N % vor Ort`, `Remote: N %`, and office or
/// remote days of a week (`zwei Tage vor Ort` = 60 %).
fn stated_shares(folded: &str) -> Vec<u64> {
    let mut out = Vec::new();
    let remote = has_remote(folded);
    let onsite = has_onsite(folded);
    let bytes = folded.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || (i > 0 && bytes[i - 1].is_ascii_alphanumeric()) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let Ok(n) = folded[start..i].parse::<u64>() else {
            continue;
        };
        let rest = folded[i..].trim_start();
        if let Some(after) = rest.strip_prefix('%')
            && n <= 100
        {
            // The words right after the share say what it is.
            let head = words(after).take(3).collect::<Vec<_>>().join(" ");
            let to_remote = first_at(&head, lex::REMOTE_AD_WORDS);
            let to_onsite = first_at(&head, lex::ONSITE_AD_WORDS);
            if to_onsite < to_remote {
                out.push(100 - n);
            } else if to_remote < to_onsite || remote {
                out.push(n);
            }
        }
    }
    let tokens: Vec<&str> = words(folded).collect();
    for (at, pair) in tokens.windows(2).enumerate() {
        let n = pair[0].parse::<u64>().ok().or_else(|| {
            lex::DAY_NUMBERS
                .iter()
                .find(|(word, _)| *word == pair[0])
                .map(|&(_, n)| n)
        });
        let Some(n) = n.filter(|n| (1..=WORKDAYS).contains(n)) else {
            continue;
        };
        if !lex::DAY_WORDS.contains(&pair[1]) {
            continue;
        }
        let after = tokens[at + 2..].join(" ");
        let (days_remote, days_onsite) = if has_remote(&after) || has_onsite(&after) {
            let first_remote = first_at(&after, lex::REMOTE_AD_WORDS);
            let first_onsite = first_at(&after, lex::ONSITE_AD_WORDS);
            (first_remote < first_onsite, first_onsite < first_remote)
        } else {
            (remote && !onsite, onsite && !remote)
        };
        let per_day = 100 / WORKDAYS;
        if days_remote {
            out.push(n * per_day);
        } else if days_onsite {
            out.push((WORKDAYS - n) * per_day);
        }
    }
    out
}

/// Byte position of the first word of `list` in a folded text (`usize::MAX` if none).
fn first_at(folded: &str, list: &[&str]) -> usize {
    let mut best = usize::MAX;
    for entry in list {
        let mut from = 0;
        while let Some(at) = folded[from..].find(entry) {
            let start = from + at;
            let before = folded[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
            if before {
                best = best.min(start);
                break;
            }
            from = start + entry.len();
        }
    }
    best
}

/// The ad's remote share in percent (from, to), as far as it can be read.
pub(crate) fn remote_share(
    job: &JobFacts<'_>,
    segments: &[Segment],
    folded: &str,
) -> Option<(u64, u64)> {
    if let Some(p) = fact(job.facts, super::fact_key::REMOTE_PERCENT).and_then(Value::as_u64) {
        let p = p.min(100);
        return Some((p, p));
    }
    let location = fold(
        fact(job.facts, super::fact_key::LOCATION)
            .and_then(Value::as_str)
            .unwrap_or(job.location),
    );
    let mut stated: Vec<u64> = segments
        .iter()
        .flat_map(|(_, f)| stated_shares(f))
        .collect();
    let full = core_lex::FULL_REMOTE
        .iter()
        .chain(core_lex::REMOTE_FULL_EXTRA)
        .any(|w| folded.contains(w))
        || contains_word(&location, "remote");
    if full {
        stated.push(100);
    }
    if let Some(&max) = stated.iter().max() {
        return Some((max, max));
    }
    if lex::REMOTE_NONE.iter().any(|w| folded.contains(w)) {
        return Some((0, 0));
    }
    if lex::HYBRID_WORDS.iter().any(|w| folded.contains(w)) || contains_word(&location, "hybrid") {
        return Some(HYBRID_SHARE);
    }
    if segments.iter().any(|(_, f)| has_remote(f)) {
        let mixed = segments.iter().any(|(_, f)| has_remote(f) && has_onsite(f));
        return Some(if mixed {
            HYBRID_SHARE
        } else {
            REMOTE_NAMED_SHARE
        });
    }
    if segments.iter().any(|(_, f)| has_onsite(f)) || has_onsite(&location) {
        return Some((0, 0));
    }
    None
}

fn remote(wish: RemoteWish, share: Option<(u64, u64)>) -> WishResult {
    let code = ReasonCode::RemoteWish;
    let mut params = match wish {
        RemoteWish::Min(min) => json!({ "min": min, "onsite": false }),
        RemoteWish::OnSite => json!({ "min": null, "onsite": true }),
    };
    let Some((from, to)) = share else {
        return result(code, State::Unknown, WISH_REMOTE, params, Vec::new());
    };
    if from == to {
        params["share"] = json!(from);
    } else {
        params["from"] = json!(from);
        params["to"] = json!(to);
    }
    let (met, missed) = match wish {
        RemoteWish::Min(min) => (from >= min, to == 0 || to + REMOTE_MARGIN < min),
        RemoteWish::OnSite => (to <= ONSITE_MAX, from > ONSITE_MAX + REMOTE_MARGIN),
    };
    let state = if met {
        State::Met
    } else if missed {
        State::Missed
    } else {
        State::Near
    };
    result(code, state, WISH_REMOTE, params, Vec::new())
}

/// The place of `places` a folded text names first (`München, Bayern` names München).
fn first_named<'p>(folded: &str, places: impl Iterator<Item = &'p str>) -> Option<&'p str> {
    places
        .filter_map(|place| {
            let mut from = 0;
            while let Some(at) = folded[from..].find(place) {
                let start = from + at;
                let end = start + place.len();
                let before = folded[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric());
                let after = folded[end..]
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_alphanumeric());
                if before && after {
                    return Some((start, place));
                }
                from = end;
            }
            None
        })
        .min_by_key(|(start, place)| (*start, std::cmp::Reverse(place.len())))
        .map(|(_, place)| place)
}

/// The first word of `text` whose folded form is `place` (else `place` itself).
fn as_written(text: &str, place: &str) -> String {
    words(text)
        .find(|w| fold(w) == place)
        .map_or_else(|| place.to_owned(), str::to_owned)
}

fn region(places: &[String], ad: &Ad<'_>, share: Option<(u64, u64)>) -> WishResult {
    let code = ReasonCode::RegionWish;
    let from = share.map_or(0, |(from, _)| from);
    if from >= 100 {
        return result(
            code,
            State::Met,
            WISH_REGION,
            json!({ "remote": true }),
            Vec::new(),
        );
    }
    let job = ad.job;
    let raw = fact(job.facts, super::fact_key::LOCATION)
        .and_then(Value::as_str)
        .unwrap_or(job.location);
    // Where the ad names its place: the location, location lines, on-site sentences.
    let mut sources: Vec<(String, Option<Range<usize>>, &str)> = vec![(fold(raw), None, raw)];
    for (range, f) in ad.segments {
        let line = core_lex::LOCATION_LINES.iter().any(|w| f.starts_with(w));
        let onsite = core_lex::ONSITE_WORDS.iter().any(|w| f.contains(w));
        if line || onsite {
            let written = job.text.get(range.clone()).unwrap_or("");
            sources.push((f.clone(), Some(range.clone()), written));
        }
    }
    let spans = |range: &Option<Range<usize>>| range.clone().into_iter().collect::<Vec<_>>();
    for (folded, range, written) in &sources {
        if let Some(place) = first_named(folded, places.iter().map(String::as_str)) {
            let params = json!({ "location": as_written(written, place), "remote": false });
            return result(code, State::Met, WISH_REGION, params, spans(range));
        }
    }
    for (folded, range, written) in &sources {
        if let Some(place) = first_named(folded, KNOWN_PLACES.iter().copied()) {
            let state = if from >= REGION_REMOTE_NEAR {
                State::Near
            } else {
                State::Missed
            };
            let params = json!({ "location": as_written(written, place), "remote": false });
            return result(code, state, WISH_REGION, params, spans(range));
        }
    }
    result(code, State::Unknown, WISH_REGION, json!({}), Vec::new())
}

fn industry(wished: &[Industry], ad: &Ad<'_>) -> WishResult {
    let code = ReasonCode::IndustryWish;
    let job = ad.job;
    let mut sources: Vec<(&str, Option<Range<usize>>)> = vec![(job.title, None)];
    // On the freelance portals the company is the agency that posts the project.
    if job.portal == Portal::LinkedIn {
        sources.push((ad.company, None));
    }
    sources.extend(
        ad.context
            .iter()
            .filter_map(|r| job.text.get(r.clone()).map(|t| (t, Some(r.clone())))),
    );
    let mut named: Vec<(&'static str, String, Option<Range<usize>>)> = Vec::new();
    for (text, range) in &sources {
        named.extend(
            industries_in(text)
                .into_iter()
                .map(|(key, word)| (key, word, range.clone())),
        );
    }
    let hit = named.iter().find_map(|(key, word, range)| {
        wished
            .iter()
            .find(|w| w.keys.contains(key))
            .map(|w| (w.text.clone(), word.clone(), range.clone()))
    });
    // A wish the word list does not know: all its atoms in one text.
    let hit = hit.or_else(|| {
        wished
            .iter()
            .filter(|w| w.keys.is_empty() && !w.atoms.is_empty())
            .find_map(|w| {
                sources.iter().find_map(|(text, range)| {
                    let found = atoms::atoms(text, ad.vocab);
                    w.atoms
                        .iter()
                        .all(|a| found.iter().any(|f| atoms::fit(f, a) == Fit::Equal))
                        .then(|| (w.text.clone(), w.text.clone(), range.clone()))
                })
            })
    });
    if let Some((wish, word, range)) = hit {
        let params = json!({ "industry": word, "wish": wish });
        return result(
            code,
            State::Met,
            WISH_INDUSTRY,
            params,
            range.into_iter().collect(),
        );
    }
    if let Some((_, word, range)) = named.first() {
        let params = json!({ "industry": word });
        return result(
            code,
            State::Missed,
            WISH_INDUSTRY,
            params,
            range.clone().into_iter().collect(),
        );
    }
    result(code, State::Unknown, WISH_INDUSTRY, json!({}), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::facts::segments;

    #[test]
    fn remote_wishes_and_old_free_text() {
        let wish = |v: Value| remote_wish(&v);
        assert_eq!(wish(json!("voll")), Some(RemoteWish::Min(REMOTE_FULL)));
        assert_eq!(
            wish(json!("ueberwiegend")),
            Some(RemoteWish::Min(REMOTE_MOSTLY))
        );
        assert_eq!(
            wish(json!("überwiegend")),
            Some(RemoteWish::Min(REMOTE_MOSTLY))
        );
        assert_eq!(
            wish(json!("teilweise")),
            Some(RemoteWish::Min(REMOTE_PARTLY))
        );
        assert_eq!(wish(json!("vor_ort")), Some(RemoteWish::OnSite));
        assert_eq!(wish(json!("mindestens 50 %")), Some(RemoteWish::Min(50)));
        assert_eq!(wish(json!("100%")), Some(RemoteWish::Min(100)));
        assert_eq!(wish(json!(0)), Some(RemoteWish::OnSite));
        assert_eq!(wish(json!("gern")), None);
        assert_eq!(wish(json!(true)), None);
    }

    fn share(location: &str, text: &str, facts: Option<&Value>) -> Option<(u64, u64)> {
        let job = JobFacts {
            title: "Controller",
            text,
            location,
            portal: Portal::LinkedIn,
            facts,
            posted: None,
        };
        remote_share(&job, &segments(text), &fold(text))
    }

    #[test]
    fn remote_shares_of_ads() {
        let s = |text: &str| share("Hamburg", text, None);
        assert_eq!(
            s("Einsatz zu 100 % remote aus Deutschland"),
            Some((100, 100))
        );
        assert_eq!(s("Remote: 80 %"), Some((80, 80)));
        assert_eq!(
            s("Einsatz zu 60 % remote, zwei Tage vor Ort in München"),
            Some((60, 60))
        );
        assert_eq!(s("Einsatz zu 100 % vor Ort in Hamburg"), Some((0, 0)));
        assert_eq!(s("80 % vor Ort, 20 % remote"), Some((20, 20)));
        assert_eq!(
            s("Hybrid: 3 days remote per week, 2 days on site"),
            Some((60, 60))
        );
        assert_eq!(
            s("Hybrides Arbeiten mit zwei Tagen im Homeoffice pro Woche"),
            Some((40, 40))
        );
        assert_eq!(
            s("Hybrid mit drei Tagen pro Woche vor Ort in Wiesbaden"),
            Some((40, 40))
        );
        assert_eq!(
            s("Einsatz vor Ort in Berlin, zwei Tage pro Woche"),
            Some((60, 60))
        );
        assert_eq!(
            s(
                "Die Arbeit erfolgt überwiegend vor Ort, ein Remote-Anteil von bis zu 40 % ist möglich."
            ),
            Some((40, 40))
        );
        assert_eq!(s("Hybrides Arbeiten nach Absprache"), Some(HYBRID_SHARE));
        assert_eq!(s("Remote möglich"), Some(REMOTE_NAMED_SHARE));
        assert_eq!(s("Remote und vor Ort beim Kunden"), Some(HYBRID_SHARE));
        assert_eq!(s("Einsatz vor Ort in Hamburg"), Some((0, 0)));
        assert_eq!(
            s("Remote ist nicht möglich, Einsatz beim Kunden"),
            Some((0, 0))
        );
        assert_eq!(
            s("Reisebereitschaft (ca. 20 %) und Erfahrung in der Automobilindustrie"),
            None
        );
        assert_eq!(s("30 Tage Urlaub"), None);
        assert_eq!(
            share("München (Remote)", "Controlling", None),
            Some((100, 100))
        );
        assert_eq!(
            share("Hamburg (Hybrid)", "Controlling", None),
            Some(HYBRID_SHARE)
        );
        let facts = json!({ "remotePercent": 40 });
        assert_eq!(
            share("Hamburg", "Einsatz zu 100 % remote", Some(&facts)),
            Some((40, 40))
        );
    }

    #[test]
    fn places_in_the_order_the_text_names_them() {
        let places = ["bayern", "munchen"];
        assert_eq!(
            first_named("munchen, bayern, deutschland", places.iter().copied()),
            Some("munchen")
        );
        assert_eq!(first_named("munchenstein", places.iter().copied()), None);
        assert_eq!(as_written("München, Bayern", "munchen"), "München");
        assert!(expand("bayern").contains(&"nurnberg".to_owned()));
        let germany = expand("deutschland");
        assert!(germany.contains(&"hamburg".to_owned()) && germany.contains(&"germany".to_owned()));
        assert!(expand("osterreich").contains(&"wien".to_owned()));
    }

    #[test]
    fn industries_by_whole_words_and_word_starts() {
        let keys = |text: &str| {
            industries_in(text)
                .into_iter()
                .map(|(k, _)| k)
                .collect::<Vec<_>>()
        };
        assert_eq!(keys("Ein familiengeführter Maschinenbauer"), ["machinery"]);
        assert_eq!(
            keys("ein Unternehmen der Automobilzulieferindustrie"),
            ["automotive"]
        );
        assert_eq!(keys("Für eine Bank migrieren wir"), ["banking"]);
        assert!(keys("Weiterentwicklung der Bankenbeziehungen").is_empty());
        assert!(keys("Datenbanken und Medienbrüche").is_empty());
        assert_eq!(keys("Private Equity"), ["privateEquity"]);
        assert_eq!(keys("Rheingau Versicherung AG"), ["insurance"]);
        assert!(keys("Handelsgesetzbuch und Handelsrecht").is_empty());
    }

    #[test]
    fn tables_are_folded() {
        let folded = |w: &str| fold(w) == w;
        for (word, _) in lex::INDUSTRIES.iter().chain(lex::INDUSTRY_WORDS) {
            assert!(folded(word), "{word}");
        }
        for (region, places) in lex::REGION_PLACES {
            assert!(
                folded(region) && places.iter().all(|p| folded(p)),
                "{region}"
            );
        }
        for list in [
            lex::LEAD_WORDS,
            lex::LEVEL_WORDS,
            lex::ROLE_CONTRACT_WORDS,
            lex::REMOTE_AD_WORDS,
            lex::ONSITE_AD_WORDS,
            lex::REMOTE_NONE,
            lex::HYBRID_WORDS,
            lex::TASK_HEADINGS,
        ] {
            assert!(list.iter().all(|w| folded(w)), "{list:?}");
        }
        for (phrase, replacement) in lex::ROLE_PHRASES {
            assert!(folded(phrase) && folded(replacement), "{phrase}");
        }
        for (word, level) in lex::REMOTE_WISH_WORDS {
            assert!(folded(word), "{word}");
            assert!(
                ["onsite", "full", "mostly", "partly"].contains(level),
                "{level}"
            );
        }
    }
}
