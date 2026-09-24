//! The profile editor's form: exactly the profile keys the matching engine and the external
//! `job-matching` skill use, read the way the engine reads them, and written back by merging
//! into the JSON the form came from. Only fields that differ from what the editor started
//! with are written; every other key, its value and the order of the keys stay as they were.
//!
//! The profile keys are an external contract (skill, hand-made profiles) - do not translate.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use super::json::Json;
use crate::error::InvalidInput;
use crate::matching;
use crate::matching::facts::Availability;
use crate::matching::lexicon::{self, engine as lex};

const KEY_NAME: &str = "name";
const KEY_TITLE: &str = "titel";
const KEY_DEGREE: &str = "abschluss";
const KEY_EDUCATION: &str = "ausbildung";
const KEY_COMPETENCES: &str = "kernkompetenzen";
const KEY_COMPETENCE: &str = "kompetenz";
const KEY_TOOLS: &str = "methoden_tools";
const KEY_CERTIFICATES: &str = "zertifizierungen";
const KEY_INDUSTRIES: &str = "branchen";
const KEY_STRENGTHS: &str = "alleinstellungsmerkmale";
const KEY_KEYWORDS: &str = "keywords";
const KEY_FOCUS: &str = "schwerpunkte";
const KEY_ROLES: &str = "wunschrollen";
/// Wishes inside `einsatzpraeferenzen` (they nudge the score, they never exclude).
const KEY_RATE_WISH: &str = "tagessatz_wunsch";
const KEY_REMOTE: &str = "remote";
const KEY_REGIONS: &str = "regionen";
const KEY_WISH_INDUSTRIES: &str = "branchen";
/// At most this many competences are marked as the ones that matter most.
pub const MAX_FOCUS: usize = 5;

/// Upper bounds of the numbers (anything above is a typing error).
const MAX_YEARS: u32 = 70;
const MAX_DAY_RATE: u32 = 100_000;
const MAX_SALARY: u32 = 10_000_000;
const MAX_PERCENT: u32 = 100;
/// A single value and a list stay within what a profile ever holds.
const MAX_TEXT: usize = 1_000;
const MAX_ITEMS: usize = 300;

/// The editable profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileForm {
    /// `name` (the skill names the consultant with it; the engine never reads it).
    pub name: String,
    /// `titel`, the professional role.
    pub title: String,
    /// `kernkompetenzen[]` with `kompetenz`, `jahre` and `auch`.
    pub competences: Vec<ProfileCompetence>,
    /// `alleinstellungsmerkmale[]`.
    pub strengths: Vec<String>,
    /// `keywords[]`.
    pub keywords: Vec<String>,
    /// `berufserfahrung_jahre`.
    pub years: Option<u32>,
    /// `abschluss` and `ausbildung[].abschluss`.
    pub degrees: Vec<String>,
    /// `branchen[].branche`.
    pub industries: Vec<String>,
    /// `methoden_tools[].name`.
    pub tools: Vec<String>,
    /// `zertifizierungen[].name`.
    pub certificates: Vec<String>,
    /// `sprachen[]` with `sprache` and `niveau`.
    pub languages: Vec<ProfileLanguage>,
    /// `schwerpunkte[]`: the competences that matter most (at most five).
    pub focus: Vec<String>,
    /// `wunschrollen[]`: the roles the consultant is looking for.
    pub roles: Vec<String>,
    /// `einsatzpraeferenzen`: wishes, they only nudge the score.
    pub wishes: ProfileWishes,
    /// `harte_kriterien` (with the `einsatzpraeferenzen` fallbacks).
    pub criteria: ProfileCriteria,
}

/// Wishes of the consultant (`einsatzpraeferenzen`): unlike the hard criteria they never
/// exclude a job.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileWishes {
    /// `tagessatz_wunsch`, EUR per day.
    pub day_rate: Option<u32>,
    /// `remote`.
    pub remote: Option<RemoteWish>,
    /// `regionen[]`, preferred cities or regions.
    pub regions: Vec<String>,
    /// `branchen[]` inside `einsatzpraeferenzen`, preferred industries.
    pub industries: Vec<String>,
}

/// How much remote work the consultant wishes for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum RemoteWish {
    Full,
    Mostly,
    Partly,
    OnSite,
}

impl RemoteWish {
    /// The value as the profile writes it (`einsatzpraeferenzen.remote`).
    pub fn text(self) -> &'static str {
        match self {
            RemoteWish::Full => "voll",
            RemoteWish::Mostly => "ueberwiegend",
            RemoteWish::Partly => "teilweise",
            RemoteWish::OnSite => "vor_ort",
        }
    }

    /// The written value, or the free text older profiles have (`mindestens 50 %`,
    /// `100 % remote`, `hybrid`, `vor Ort`): a share of 100 is full, above half mostly,
    /// above zero partly, zero on site. `None` when it says neither.
    pub fn read(text: &str) -> Option<RemoteWish> {
        let folded = text
            .trim()
            .to_lowercase()
            .replace('\u{fc}', "ue")
            .replace('\u{e4}', "ae");
        let exact = [
            RemoteWish::Full,
            RemoteWish::Mostly,
            RemoteWish::Partly,
            RemoteWish::OnSite,
        ]
        .into_iter()
        .find(|wish| wish.text() == folded);
        if exact.is_some() {
            return exact;
        }
        let digits: String = folded
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(share) = digits.parse::<u32>() {
            return Some(match share {
                0 => RemoteWish::OnSite,
                1..=50 => RemoteWish::Partly,
                51..=99 => RemoteWish::Mostly,
                _ => RemoteWish::Full,
            });
        }
        let has = |words: &[&str]| words.iter().any(|w| folded.contains(w));
        if has(&[
            "vor ort", "vor_ort", "vor-ort", "onsite", "on-site", "praesenz",
        ]) {
            Some(RemoteWish::OnSite)
        } else if has(&["ueberwiegend", "mostly", "mainly"]) {
            Some(RemoteWish::Mostly)
        } else if has(&["teilweise", "hybrid", "partly", "partial"]) {
            Some(RemoteWish::Partly)
        } else if has(&["voll", "full", "komplett", "remote"]) {
            Some(RemoteWish::Full)
        } else {
            None
        }
    }
}

/// One core competence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileCompetence {
    pub name: String,
    pub years: Option<u32>,
    /// Other words for the same competence (`auch`).
    pub aliases: Vec<String>,
    /// Index of the entry in the JSON the form was read from; `null` for a new one.
    pub origin: Option<u32>,
}

/// One language with its level.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileLanguage {
    pub language: String,
    pub level: Option<LanguageLevel>,
    /// Index of the entry in the JSON the form was read from; `null` for a new one.
    pub origin: Option<u32>,
}

/// CEFR level or native.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum LanguageLevel {
    A1,
    A2,
    B1,
    B2,
    C1,
    C2,
    Native,
}

impl LanguageLevel {
    fn from_engine(level: u8) -> Option<LanguageLevel> {
        Some(match level {
            1 => LanguageLevel::A1,
            2 => LanguageLevel::A2,
            3 => LanguageLevel::B1,
            4 => LanguageLevel::B2,
            5 => LanguageLevel::C1,
            6 => LanguageLevel::C2,
            7 => LanguageLevel::Native,
            _ => return None,
        })
    }

    /// The level as the profile writes it (`niveau`).
    fn text(self) -> &'static str {
        match self {
            LanguageLevel::A1 => "A1",
            LanguageLevel::A2 => "A2",
            LanguageLevel::B1 => "B1",
            LanguageLevel::B2 => "B2",
            LanguageLevel::C1 => "C1",
            LanguageLevel::C2 => "C2",
            LanguageLevel::Native => "Muttersprache",
        }
    }
}

/// The hard criteria (a missing value switches its rule off).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileCriteria {
    /// `min_tagessatz` (read with the fallback `einsatzpraeferenzen.tagessatz_ab`).
    pub min_day_rate: Option<u32>,
    /// `laender`, ISO codes (`DE`, `AT`, ...).
    pub countries: Vec<String>,
    /// `ausgeschlossene_vertragsarten` contains `anue`.
    pub no_anue: bool,
    /// `verfuegbar_ab`.
    pub available: ProfileAvailability,
    /// `remote_ausserhalb_erlaubt`.
    pub remote_outside: bool,
    /// `zielprofil_min_jahre`.
    pub target_years: Option<u32>,
    /// `min_jahresgehalt` (permanent roles).
    pub min_salary: Option<u32>,
    /// `festanstellung_orte` (permanent roles).
    pub permanent_places: Vec<String>,
    /// `festanstellung_remote_min`, percent (permanent roles).
    pub permanent_remote_min: Option<u32>,
}

/// When the consultant is available.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum ProfileAvailability {
    #[default]
    Unset,
    Now,
    /// From a day (`date` as `YYYY-MM-DD`).
    From {
        date: String,
    },
}

impl ProfileForm {
    /// Values trimmed, empty ones dropped, list entries once (case-insensitive), zero where
    /// the engine ignores it as none.
    #[must_use]
    pub fn normalized(&self) -> ProfileForm {
        let c = &self.criteria;
        ProfileForm {
            name: self.name.trim().to_owned(),
            title: self.title.trim().to_owned(),
            competences: self
                .competences
                .iter()
                .filter(|row| !row.name.trim().is_empty())
                .map(|row| ProfileCompetence {
                    name: row.name.trim().to_owned(),
                    years: row.years,
                    aliases: clean(&row.aliases),
                    origin: row.origin,
                })
                .collect(),
            strengths: clean(&self.strengths),
            keywords: clean(&self.keywords),
            years: self.years,
            degrees: clean(&self.degrees),
            industries: clean(&self.industries),
            tools: clean(&self.tools),
            certificates: clean(&self.certificates),
            languages: self
                .languages
                .iter()
                .filter(|row| !row.language.trim().is_empty())
                .map(|row| ProfileLanguage {
                    language: row.language.trim().to_owned(),
                    level: row.level,
                    origin: row.origin,
                })
                .collect(),
            focus: clean(&self.focus),
            roles: clean(&self.roles),
            wishes: ProfileWishes {
                day_rate: self.wishes.day_rate.filter(|n| *n > 0),
                remote: self.wishes.remote,
                regions: clean(&self.wishes.regions),
                industries: clean(&self.wishes.industries),
            },
            criteria: ProfileCriteria {
                min_day_rate: c.min_day_rate.filter(|n| *n > 0),
                countries: clean(
                    &c.countries
                        .iter()
                        .map(|code| code.trim().to_uppercase())
                        .collect::<Vec<_>>(),
                ),
                no_anue: c.no_anue,
                available: match &c.available {
                    ProfileAvailability::From { date } => ProfileAvailability::From {
                        date: date.trim().to_owned(),
                    },
                    other => other.clone(),
                },
                remote_outside: c.remote_outside,
                target_years: c.target_years.filter(|n| *n > 0),
                min_salary: c.min_salary.filter(|n| *n > 0),
                permanent_places: clean(&c.permanent_places),
                permanent_remote_min: c.permanent_remote_min.filter(|n| *n > 0),
            },
        }
    }

    /// Anything about the person (wishes and hard criteria are preferences, not a profile).
    pub fn has_content(&self) -> bool {
        let empty = ProfileForm {
            roles: self.roles.clone(),
            wishes: self.wishes.clone(),
            criteria: self.criteria.clone(),
            ..ProfileForm::default()
        };
        self.normalized() != empty.normalized()
    }
}

/// Trimmed, non-empty, each entry once (case-insensitive, the first spelling wins).
fn clean(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let text = item.trim();
        if !text.is_empty() && !out.iter().any(|o| o.to_lowercase() == text.to_lowercase()) {
            out.push(text.to_owned());
        }
    }
    out
}

/// The form normalized, or the first value out of range (`field` names it).
pub(crate) fn validate(form: &ProfileForm) -> Result<ProfileForm, InvalidInput> {
    let form = form.normalized();
    let fail = |field: &str| InvalidInput::ProfileValue {
        field: field.to_owned(),
    };
    let within = |value: Option<u32>, max: u32, field: &str| match value {
        Some(n) if n > max => Err(fail(field)),
        _ => Ok(()),
    };
    let texts = |items: &[String], field: &str| {
        if items.len() > MAX_ITEMS || items.iter().any(|t| t.chars().count() > MAX_TEXT) {
            Err(fail(field))
        } else {
            Ok(())
        }
    };
    let c = &form.criteria;
    within(form.years, MAX_YEARS, "years")?;
    within(c.min_day_rate, MAX_DAY_RATE, "minDayRate")?;
    within(c.target_years, MAX_YEARS, "targetYears")?;
    within(c.min_salary, MAX_SALARY, "minSalary")?;
    within(c.permanent_remote_min, MAX_PERCENT, "permanentRemoteMin")?;
    within(form.wishes.day_rate, MAX_DAY_RATE, "wishDayRate")?;
    if form.focus.len() > MAX_FOCUS {
        return Err(fail("focus"));
    }
    texts(&form.focus, "focus")?;
    texts(&form.roles, "roles")?;
    texts(&form.wishes.regions, "regions")?;
    texts(&form.wishes.industries, "wishIndustries")?;
    texts(std::slice::from_ref(&form.name), "name")?;
    texts(std::slice::from_ref(&form.title), "title")?;
    let names: Vec<String> = form.competences.iter().map(|r| r.name.clone()).collect();
    texts(&names, "competences")?;
    for row in &form.competences {
        within(row.years, MAX_YEARS, "competences")?;
        texts(&row.aliases, "competences")?;
    }
    texts(&form.strengths, "strengths")?;
    texts(&form.keywords, "keywords")?;
    texts(&form.degrees, "degrees")?;
    texts(&form.industries, "industries")?;
    texts(&form.tools, "tools")?;
    texts(&form.certificates, "certificates")?;
    let languages: Vec<String> = form.languages.iter().map(|r| r.language.clone()).collect();
    texts(&languages, "languages")?;
    texts(&c.countries, "countries")?;
    texts(&c.permanent_places, "permanentPlaces")?;
    if let ProfileAvailability::From { date } = &c.available
        && date.parse::<Date>().is_err()
    {
        return Err(fail("available"));
    }
    Ok(form)
}

// ------------------------------------------------------------------------------ reading

/// The item keys the engine reads from a profile list (`methoden_tools` -> `name`, ...).
fn sub_keys(list: &str) -> &'static [&'static str] {
    lexicon::PROFILE_LISTS
        .iter()
        .find(|(key, _)| *key == list)
        .map_or(&[], |(_, subs)| *subs)
}

/// The text of a list item: a string, or the first of `subs` in an object.
fn item_text<'a>(item: &'a Json, subs: &[&str]) -> Option<&'a str> {
    let text = match item {
        Json::String(text) => text.as_str(),
        Json::Object(_) => subs.iter().find_map(|key| item.get(key)?.as_str())?,
        _ => return None,
    };
    Some(text.trim()).filter(|t| !t.is_empty())
}

fn items<'a>(doc: &'a Json, key: &str) -> &'a [Json] {
    doc.get(key).and_then(Json::as_array).unwrap_or_default()
}

fn text_at(doc: &Json, key: &str) -> String {
    doc.get(key)
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn read_list(doc: &Json, key: &str) -> Vec<String> {
    let subs = sub_keys(key);
    items(doc, key)
        .iter()
        .filter_map(|item| item_text(item, subs))
        .map(str::to_owned)
        .collect()
}

fn whole(value: &Json) -> Option<u32> {
    match value {
        Json::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

/// The first of `keys` with a whole number (like the engine).
fn number_at(item: &Json, keys: &[&str]) -> Option<u32> {
    keys.iter().find_map(|key| whole(item.get(key)?))
}

/// Alternative terms of a competence from every alias key (like the engine).
fn aliases_of(item: &Json) -> Vec<String> {
    lexicon::KEYS_ALIASES
        .iter()
        .flat_map(|key| items(item, key))
        .filter_map(Json::as_str)
        .map(|t| t.trim().to_owned())
        .collect()
}

fn index(i: usize) -> Option<u32> {
    u32::try_from(i).ok()
}

fn read_competences(doc: &Json) -> Vec<ProfileCompetence> {
    items(doc, KEY_COMPETENCES)
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            Some(ProfileCompetence {
                name: item_text(item, &[KEY_COMPETENCE])?.to_owned(),
                years: number_at(item, lex::KEYS_YEARS),
                aliases: aliases_of(item),
                origin: index(i),
            })
        })
        .collect()
}

fn read_languages(doc: &Json) -> Vec<ProfileLanguage> {
    items(doc, lex::KEY_LANGUAGES)
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            Some(ProfileLanguage {
                language: item_text(item, &[lex::KEY_LANGUAGE])?.to_owned(),
                level: item
                    .get(lex::KEY_LEVEL)
                    .and_then(Json::as_str)
                    .and_then(matching::language_level)
                    .and_then(LanguageLevel::from_engine),
                origin: index(i),
            })
        })
        .collect()
}

/// `abschluss` (a text or a list of texts), then `ausbildung[].abschluss`.
fn read_degrees(doc: &Json) -> Vec<String> {
    let top = match doc.get(KEY_DEGREE) {
        Some(Json::String(text)) => vec![text.clone()],
        Some(Json::Array(items)) => items
            .iter()
            .filter_map(Json::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    let education = items(doc, KEY_EDUCATION)
        .iter()
        .filter_map(|item| item.get(KEY_DEGREE)?.as_str())
        .map(str::to_owned);
    top.into_iter().chain(education).collect()
}

/// Texts of a list without list items the form cannot show.
fn read_texts(list: Option<&Json>) -> Vec<String> {
    list.and_then(Json::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item_text(item, &[]))
        .map(str::to_owned)
        .collect()
}

fn read_wishes(doc: &Json) -> ProfileWishes {
    let prefs = doc.get(lexicon::KEY_PREFERENCES);
    let at = |key: &str| prefs.and_then(|p| p.get(key));
    ProfileWishes {
        day_rate: at(KEY_RATE_WISH).and_then(whole),
        remote: at(KEY_REMOTE)
            .and_then(Json::as_str)
            .and_then(RemoteWish::read),
        regions: read_texts(at(KEY_REGIONS)),
        industries: read_texts(at(KEY_WISH_INDUSTRIES)),
    }
}

fn read_criteria(doc: &Json) -> ProfileCriteria {
    let c = matching::hard_criteria(&doc.to_value());
    ProfileCriteria {
        min_day_rate: c.min_rate.and_then(|n| u32::try_from(n).ok()),
        countries: c.countries.unwrap_or_default(),
        no_anue: c.anue_excluded,
        available: match c.available {
            Availability::Unset => ProfileAvailability::Unset,
            Availability::Now => ProfileAvailability::Now,
            Availability::From(day) => ProfileAvailability::From {
                date: day.to_string(),
            },
        },
        remote_outside: c.remote_outside == Some(true),
        target_years: c.target_years,
        min_salary: c.min_salary.and_then(|n| u32::try_from(n).ok()),
        permanent_places: c.places.unwrap_or_default(),
        permanent_remote_min: c.remote_min.and_then(|n| u32::try_from(n).ok()),
    }
}

/// The form of a profile document, read the way the engine reads it.
pub(crate) fn read(doc: &Json) -> ProfileForm {
    ProfileForm {
        name: text_at(doc, KEY_NAME),
        title: text_at(doc, KEY_TITLE),
        competences: read_competences(doc),
        strengths: read_list(doc, KEY_STRENGTHS),
        keywords: read_list(doc, KEY_KEYWORDS),
        years: number_at(doc, lexicon::KEYS_TOTAL_YEARS),
        degrees: read_degrees(doc),
        industries: read_list(doc, KEY_INDUSTRIES),
        tools: read_list(doc, KEY_TOOLS),
        certificates: read_list(doc, KEY_CERTIFICATES),
        languages: read_languages(doc),
        focus: read_texts(doc.get(KEY_FOCUS)),
        roles: read_texts(doc.get(KEY_ROLES)),
        wishes: read_wishes(doc),
        criteria: read_criteria(doc),
    }
    .normalized()
}

// ------------------------------------------------------------------------------ writing

/// Writes into `doc` every field of `after` that differs from `before` (the form as the
/// editor received it). Fields in canonical order, so a new profile reads like the template
/// of the skill; a key that is new in an existing profile goes to its end.
pub(crate) fn merge(doc: &mut Json, before: &ProfileForm, after: &ProfileForm) {
    let before = before.normalized();
    let after = after.normalized();
    if !doc.is_object() {
        *doc = Json::object();
    }
    if after.name != before.name {
        set_text(doc, KEY_NAME, &after.name);
    }
    if after.title != before.title {
        set_text(doc, KEY_TITLE, &after.title);
    }
    if after.roles != before.roles {
        write_list(doc, KEY_ROLES, &after.roles);
    }
    if after.years != before.years {
        write_number(doc, lexicon::KEYS_TOTAL_YEARS, after.years);
    }
    if after.degrees != before.degrees {
        write_degrees(doc, &after.degrees);
    }
    if after.competences != before.competences {
        write_competences(doc, &before.competences, &after.competences);
    }
    if after.focus != before.focus {
        write_list(doc, KEY_FOCUS, &after.focus);
    }
    for (key, old, new) in [
        (KEY_TOOLS, &before.tools, &after.tools),
        (KEY_CERTIFICATES, &before.certificates, &after.certificates),
        (KEY_INDUSTRIES, &before.industries, &after.industries),
    ] {
        if new != old {
            write_list(doc, key, new);
        }
    }
    if after.languages != before.languages {
        write_languages(doc, &before.languages, &after.languages);
    }
    for (key, old, new) in [
        (KEY_STRENGTHS, &before.strengths, &after.strengths),
        (KEY_KEYWORDS, &before.keywords, &after.keywords),
    ] {
        if new != old {
            write_list(doc, key, new);
        }
    }
    write_wishes(doc, &before.wishes, &after.wishes);
    write_criteria(doc, &before.criteria, &after.criteria);
}

/// The wishes inside `einsatzpraeferenzen` (created only when there is something to write).
fn write_wishes(doc: &mut Json, before: &ProfileWishes, after: &ProfileWishes) {
    let section = lexicon::KEY_PREFERENCES;
    if after.day_rate != before.day_rate {
        if let Some(rate) = after.day_rate {
            doc.object_mut(section)
                .set(KEY_RATE_WISH, Json::number(rate));
        } else {
            remove_in(doc, section, KEY_RATE_WISH);
        }
    }
    if after.remote != before.remote {
        if let Some(wish) = after.remote {
            doc.object_mut(section)
                .set(KEY_REMOTE, Json::text(wish.text()));
        } else {
            remove_in(doc, section, KEY_REMOTE);
        }
    }
    for (key, old, new) in [
        (KEY_REGIONS, &before.regions, &after.regions),
        (KEY_WISH_INDUSTRIES, &before.industries, &after.industries),
    ] {
        if new == old {
            continue;
        }
        if new.is_empty() {
            remove_in(doc, section, key);
        } else {
            // Plain texts, whatever the same key means at the top level.
            write_texts_list(doc.object_mut(section), key, &[], new);
        }
    }
}

fn set_text(doc: &mut Json, key: &str, text: &str) {
    if text.is_empty() {
        doc.remove(key);
    } else {
        doc.set(key, Json::text(text));
    }
}

fn put_list(doc: &mut Json, key: &str, items: Vec<Json>) {
    if items.is_empty() {
        doc.remove(key);
    } else {
        doc.set(key, Json::Array(items));
    }
}

/// A number under the first of `keys` that is there (else the first); `None` removes all of
/// them, so no alias brings the old value back.
fn write_number(item: &mut Json, keys: &[&str], value: Option<u32>) {
    match value {
        Some(n) => {
            let key = keys
                .iter()
                .find(|k| item.get(k).is_some())
                .unwrap_or(&keys[0]);
            item.set(key, Json::number(n));
        }
        None => {
            for key in keys {
                item.remove(key);
            }
        }
    }
}

/// Texts under the first of `keys` that is there (else the first); the other keys go, since
/// the engine reads all of them.
fn write_texts(item: &mut Json, keys: &[&str], texts: &[String]) {
    let key = *keys
        .iter()
        .find(|k| item.get(k).is_some())
        .unwrap_or(&keys[0]);
    for other in keys.iter().filter(|k| **k != key) {
        item.remove(other);
    }
    if texts.is_empty() {
        item.remove(key);
    } else {
        item.set(key, Json::texts(texts));
    }
}

/// A top-level list of the profile, items read as the engine reads them.
fn write_list(doc: &mut Json, key: &str, texts: &[String]) {
    write_texts_list(doc, key, sub_keys(key), texts);
}

/// A list of texts: items with the same text stay as they are (with their other keys), new
/// ones follow the style of the list, items the form cannot show stay at the end.
fn write_texts_list(doc: &mut Json, key: &str, subs: &[&str], texts: &[String]) {
    let old = items(doc, key).to_vec();
    let mut used = vec![false; old.len()];
    let mut out = Vec::new();
    for text in texts {
        let same = old
            .iter()
            .enumerate()
            .find(|(i, item)| !used[*i] && item_text(item, subs) == Some(text.as_str()))
            .map(|(i, _)| i);
        match same {
            Some(i) => {
                used[i] = true;
                out.push(old[i].clone());
            }
            None => out.push(new_item(&old, subs, text)),
        }
    }
    out.extend(
        old.iter()
            .enumerate()
            .filter(|(i, item)| !used[*i] && item_text(item, subs).is_none())
            .map(|(_, item)| item.clone()),
    );
    put_list(doc, key, out);
}

/// A new list item: a text in a list of texts, else an object under the key the list uses.
fn new_item(old: &[Json], subs: &[&str], text: &str) -> Json {
    let Some(first) = subs.first() else {
        return Json::text(text);
    };
    if !old.is_empty() && old.iter().all(|item| matches!(item, Json::String(_))) {
        return Json::text(text);
    }
    let key = old
        .iter()
        .find_map(|item| subs.iter().find(|k| item.get(k).is_some()))
        .unwrap_or(first);
    let mut item = Json::object();
    item.set(key, Json::text(text));
    item
}

/// Rows keep their entry (`origin`) with all its other keys; rows without one are new.
fn write_rows<R: PartialEq>(
    doc: &mut Json,
    key: &str,
    main: &str,
    rows: (&[R], &[R]),
    origin: impl Fn(&R) -> Option<u32>,
    update: impl Fn(&Json, Option<&R>, &R) -> Json,
) {
    let (before, after) = rows;
    let old = items(doc, key).to_vec();
    let mut used = vec![false; old.len()];
    let mut out = Vec::new();
    for row in after {
        let slot = origin(row)
            .and_then(|o| usize::try_from(o).ok())
            .filter(|o| *o < old.len() && !used[*o]);
        let item = match slot {
            Some(o) => {
                used[o] = true;
                let prior = before.iter().find(|b| origin(b) == origin(row));
                if prior == Some(row) {
                    old[o].clone()
                } else {
                    update(&old[o], prior, row)
                }
            }
            None => update(&Json::Null, None, row),
        };
        out.push(item);
    }
    out.extend(
        old.iter()
            .enumerate()
            .filter(|(i, item)| !used[*i] && item_text(item, &[main]).is_none())
            .map(|(_, item)| item.clone()),
    );
    put_list(doc, key, out);
}

fn write_competences(doc: &mut Json, before: &[ProfileCompetence], after: &[ProfileCompetence]) {
    write_rows(
        doc,
        KEY_COMPETENCES,
        KEY_COMPETENCE,
        (before, after),
        |row| row.origin,
        |item, prior, row| {
            if matches!(item, Json::String(_)) && row.years.is_none() && row.aliases.is_empty() {
                return Json::text(&row.name);
            }
            // A text entry that gains years or aliases becomes an object with every field.
            let (mut item, prior) = if item.is_object() {
                (item.clone(), prior)
            } else {
                (Json::object(), None)
            };
            if prior.map(|p| &p.name) != Some(&row.name) {
                item.set(KEY_COMPETENCE, Json::text(&row.name));
            }
            if prior.map(|p| p.years) != Some(row.years) {
                write_number(&mut item, lex::KEYS_YEARS, row.years);
            }
            if prior.map(|p| &p.aliases) != Some(&row.aliases) {
                write_texts(&mut item, lexicon::KEYS_ALIASES, &row.aliases);
            }
            item
        },
    );
}

fn write_languages(doc: &mut Json, before: &[ProfileLanguage], after: &[ProfileLanguage]) {
    write_rows(
        doc,
        lex::KEY_LANGUAGES,
        lex::KEY_LANGUAGE,
        (before, after),
        |row| row.origin,
        |item, prior, row| {
            if matches!(item, Json::String(_)) && row.level.is_none() {
                return Json::text(&row.language);
            }
            let (mut item, prior) = if item.is_object() {
                (item.clone(), prior)
            } else {
                (Json::object(), None)
            };
            if prior.map(|p| &p.language) != Some(&row.language) {
                item.set(lex::KEY_LANGUAGE, Json::text(&row.language));
            }
            if prior.map(|p| p.level) != Some(row.level) {
                match row.level {
                    Some(level) => item.set(lex::KEY_LEVEL, Json::text(level.text())),
                    None => {
                        item.remove(lex::KEY_LEVEL);
                    }
                }
            }
            item
        },
    );
}

/// Degrees live in `abschluss` and in `ausbildung[].abschluss`: kept ones stay where they
/// are, removed ones go, a single new one becomes `abschluss`, more go into `ausbildung`.
fn write_degrees(doc: &mut Json, degrees: &[String]) {
    let mut left: Vec<&String> = degrees.iter().collect();
    let mut take = |text: &str| match left.iter().position(|d| d.as_str() == text.trim()) {
        Some(at) => {
            left.remove(at);
            true
        }
        None => false,
    };
    if let Some(Json::Array(entries)) = doc.get_mut(KEY_EDUCATION) {
        entries.retain(|entry| match entry.get(KEY_DEGREE).and_then(Json::as_str) {
            Some(text) => take(text),
            None => true,
        });
    }
    let top = match doc.get(KEY_DEGREE) {
        Some(Json::String(text)) => Some(Json::String(text.clone())).filter(|_| take(text)),
        Some(Json::Array(entries)) => {
            let kept: Vec<Json> = entries
                .iter()
                .filter(|entry| entry.as_str().is_none_or(&mut take))
                .cloned()
                .collect();
            Some(Json::Array(kept)).filter(|k| k.as_array().is_some_and(|a| !a.is_empty()))
        }
        other => other.cloned(),
    };
    match top {
        Some(value) => doc.set(KEY_DEGREE, value),
        None => {
            doc.remove(KEY_DEGREE);
        }
    }
    if matches!(doc.get(KEY_EDUCATION), Some(Json::Array(entries)) if entries.is_empty()) {
        doc.remove(KEY_EDUCATION);
    }
    if left.is_empty() {
        return;
    }
    match doc.get(KEY_EDUCATION) {
        None if doc.get(KEY_DEGREE).is_none() && left.len() == 1 => {
            doc.set(KEY_DEGREE, Json::text(left[0]));
        }
        None | Some(Json::Array(_)) => {
            let mut entries = items(doc, KEY_EDUCATION).to_vec();
            for text in left {
                let mut entry = Json::object();
                entry.set(KEY_DEGREE, Json::text(text));
                entries.push(entry);
            }
            doc.set(KEY_EDUCATION, Json::Array(entries));
        }
        // `ausbildung` holds something else: the new degrees join `abschluss`.
        Some(_) => {
            let mut list = match doc.get(KEY_DEGREE) {
                Some(Json::Array(entries)) => entries.clone(),
                Some(Json::String(text)) => vec![Json::text(text)],
                _ => Vec::new(),
            };
            list.extend(left.into_iter().map(|t| Json::text(t)));
            doc.set(KEY_DEGREE, Json::Array(list));
        }
    }
}

fn remove_in(doc: &mut Json, section: &str, key: &str) {
    if let Some(section) = doc.get_mut(section) {
        section.remove(key);
    }
}

/// A criterion of the newer rules: where it is (German or English section and key), else
/// `harte_kriterien` with the German key; `None` removes it everywhere.
fn write_criterion(doc: &mut Json, keys: &[&str], value: Option<Json>) {
    let found = lexicon::KEY_CRITERIA_ALIASES.iter().find_map(|section| {
        let values = doc.get(section)?;
        keys.iter()
            .find(|k| values.get(k).is_some_and(|v| *v != Json::Null))
            .map(|k| (*section, *k))
    });
    match value {
        Some(value) => {
            let (section, key) = found.unwrap_or((lexicon::KEY_CRITERIA, keys[0]));
            doc.object_mut(section).set(key, value);
        }
        None => {
            for section in lexicon::KEY_CRITERIA_ALIASES {
                for key in keys {
                    remove_in(doc, section, key);
                }
            }
        }
    }
}

fn is_anue(item: &Json) -> bool {
    item.as_str()
        .is_some_and(|t| t.to_lowercase() == lexicon::CONTRACT_ANUE)
}

fn write_anue(doc: &mut Json, excluded: bool) {
    let key = lexicon::KEY_EXCLUDED_CONTRACTS;
    if excluded {
        let section = doc.object_mut(lexicon::KEY_CRITERIA);
        let mut kinds = items(section, key).to_vec();
        if !kinds.iter().any(is_anue) {
            kinds.push(Json::text(lexicon::CONTRACT_ANUE));
        }
        section.set(key, Json::Array(kinds));
    } else if let Some(section) = doc.get_mut(lexicon::KEY_CRITERIA) {
        let rest: Vec<Json> = items(section, key)
            .iter()
            .filter(|kind| !is_anue(kind))
            .cloned()
            .collect();
        put_list(section, key, rest);
    }
}

/// `2026-11-01` -> `01.11.2026`, the way the engine and the skill read a start.
fn german_date(iso: &str) -> String {
    match iso.parse::<Date>() {
        Ok(day) => format!("{:02}.{:02}.{:04}", day.day(), day.month(), day.year()),
        Err(_) => iso.to_owned(),
    }
}

fn write_criteria(doc: &mut Json, before: &ProfileCriteria, after: &ProfileCriteria) {
    let section = lexicon::KEY_CRITERIA;
    if after.min_day_rate != before.min_day_rate {
        if let Some(rate) = after.min_day_rate {
            doc.object_mut(section)
                .set(lexicon::KEY_MIN_RATE, Json::number(rate));
        } else {
            remove_in(doc, section, lexicon::KEY_MIN_RATE);
            remove_in(doc, lexicon::KEY_PREFERENCES, lexicon::KEY_RATE_FROM);
        }
    }
    if after.countries != before.countries {
        if after.countries.is_empty() {
            remove_in(doc, section, lexicon::KEY_COUNTRIES);
        } else {
            doc.object_mut(section)
                .set(lexicon::KEY_COUNTRIES, Json::texts(&after.countries));
        }
    }
    if after.no_anue != before.no_anue {
        write_anue(doc, after.no_anue);
    }
    if after.available != before.available {
        let text = match &after.available {
            ProfileAvailability::Unset => None,
            ProfileAvailability::Now => Some(lexicon::AVAILABLE_NOW.to_owned()),
            ProfileAvailability::From { date } => Some(german_date(date)),
        };
        if let Some(text) = text {
            doc.object_mut(section)
                .set(lexicon::KEY_AVAILABLE, Json::String(text));
        } else {
            remove_in(doc, section, lexicon::KEY_AVAILABLE);
            remove_in(doc, lexicon::KEY_PREFERENCES, lexicon::KEY_AVAILABLE);
        }
    }
    if after.remote_outside != before.remote_outside {
        doc.object_mut(section).set(
            lexicon::KEY_REMOTE_OUTSIDE,
            Json::Bool(after.remote_outside),
        );
    }
    if after.target_years != before.target_years {
        write_criterion(
            doc,
            lexicon::KEYS_TARGET_YEARS,
            after.target_years.map(Json::number),
        );
    }
    if after.min_salary != before.min_salary {
        write_criterion(
            doc,
            lexicon::KEYS_MIN_SALARY,
            after.min_salary.map(Json::number),
        );
    }
    if after.permanent_places != before.permanent_places {
        let places =
            (!after.permanent_places.is_empty()).then(|| Json::texts(&after.permanent_places));
        write_criterion(doc, lexicon::KEYS_PERMANENT_PLACES, places);
    }
    if after.permanent_remote_min != before.permanent_remote_min {
        write_criterion(
            doc,
            lexicon::KEYS_PERMANENT_REMOTE,
            after.permanent_remote_min.map(Json::number),
        );
    }
}
