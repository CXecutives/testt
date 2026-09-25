//! The profile editor's form: exactly the profile keys the matching engine and the external
//! `job-matching` skill use, read the way the engine reads them, and written back by merging
//! into the JSON the form came from. Only fields that differ from what the editor started
//! with are written; every other key, its value and the order of the keys stay as they were.
//! A key under its English name (`focus_areas`, `target_roles`, `preferences`, `languages`,
//! the English criteria) is read like the engine reads it and written back where it is,
//! never as a German duplicate next to it. A value the engine cannot read can be removed on
//! its own ([`UnreadableField`], "Wert entfernen").
//!
//! The profile keys are an external contract (skill, hand-made profiles) - do not translate.

use jiff::civil::Date;
use serde::{Deserialize, Serialize};

use super::json::Json;
use crate::error::InvalidInput;
use crate::matching;
use crate::matching::facts::{self, Availability};
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
/// The value of `ausgeschlossene_vertragsarten` the form writes for permanent employment.
const CONTRACT_PERMANENT: &str = "festanstellung";
/// A native language in a list under English keys (`level`).
const LEVEL_NATIVE_EN: &str = "native";
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
    /// `sprachen[]` with `sprache` and `niveau` (`languages[]` with `language` and `level`).
    pub languages: Vec<ProfileLanguage>,
    /// `schwerpunkte[]` (`focus_areas`): the competences that matter most, the first five.
    pub focus: Vec<String>,
    /// `wunschrollen[]` (`target_roles`): the roles the consultant is looking for.
    pub roles: Vec<String>,
    /// `einsatzpraeferenzen` (`preferences`): wishes, they only nudge the score.
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
    /// `tagessatz_wunsch` (`desired_day_rate`), EUR per day.
    pub day_rate: Option<u32>,
    /// `remote`.
    pub remote: Option<RemoteWish>,
    /// `regionen[]` (`regions`), preferred cities or regions.
    pub regions: Vec<String>,
    /// `branchen[]` (`industries`) inside `einsatzpraeferenzen`, preferred industries.
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
            "vor ort",
            "vor_ort",
            "vor-ort",
            "onsite",
            "on-site",
            "praesenz",
            "kein remote",
            "keine remote",
            "no remote",
            "nicht remote",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ProfileCriteria {
    /// `min_tagessatz` (read with the fallback `einsatzpraeferenzen.tagessatz_ab`).
    pub min_day_rate: Option<u32>,
    /// `laender`, ISO codes (`DE`, `AT`, ...).
    pub countries: Vec<String>,
    /// `ausgeschlossene_vertragsarten` names `anue`.
    pub no_anue: bool,
    /// `ausgeschlossene_vertragsarten` names `festanstellung`.
    pub no_permanent: bool,
    /// `verfuegbar_ab`.
    pub available: ProfileAvailability,
    /// `remote_ausserhalb_erlaubt`; missing counts as allowed, as the engine reads it.
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

impl Default for ProfileCriteria {
    fn default() -> Self {
        ProfileCriteria {
            min_day_rate: None,
            countries: Vec::new(),
            no_anue: false,
            no_permanent: false,
            available: ProfileAvailability::Unset,
            remote_outside: true,
            target_years: None,
            min_salary: None,
            permanent_places: Vec::new(),
            permanent_remote_min: None,
        }
    }
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

/// A field of the form that can hold a value the engine could not read from the file
/// (`criterionNotUnderstood`, `availabilityNotUnderstood`). "Wert entfernen" names it: saving
/// then removes every key behind it wherever the engine reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum UnreadableField {
    MinDayRate,
    Countries,
    /// `ausgeschlossene_vertragsarten` (the switches for ANÜ and permanent employment).
    Contracts,
    RemoteOutside,
    Available,
    TargetYears,
    MinSalary,
    PermanentPlaces,
    PermanentRemoteMin,
    Focus,
    Roles,
    WishDayRate,
    Remote,
    Regions,
    WishIndustries,
}

/// Where the keys of a field live.
enum Place {
    /// Top-level keys.
    Top(&'static [&'static str]),
    /// Keys of the criteria sections (`harte_kriterien`, `hard_criteria`).
    Criteria(&'static [&'static str]),
    /// Keys of the preferences sections (`einsatzpraeferenzen`, `preferences`).
    Preferences(&'static [&'static str]),
    /// Keys of both.
    Both(&'static [&'static str]),
}

impl UnreadableField {
    const ALL: [UnreadableField; 15] = [
        UnreadableField::MinDayRate,
        UnreadableField::Countries,
        UnreadableField::Contracts,
        UnreadableField::RemoteOutside,
        UnreadableField::Available,
        UnreadableField::TargetYears,
        UnreadableField::MinSalary,
        UnreadableField::PermanentPlaces,
        UnreadableField::PermanentRemoteMin,
        UnreadableField::Focus,
        UnreadableField::Roles,
        UnreadableField::WishDayRate,
        UnreadableField::Remote,
        UnreadableField::Regions,
        UnreadableField::WishIndustries,
    ];

    fn place(self) -> Place {
        match self {
            UnreadableField::MinDayRate => Place::Criteria(lexicon::KEYS_MIN_RATE),
            UnreadableField::Countries => Place::Criteria(lexicon::KEYS_COUNTRIES),
            UnreadableField::Contracts => Place::Criteria(lexicon::KEYS_EXCLUDED_CONTRACTS),
            UnreadableField::RemoteOutside => Place::Criteria(lexicon::KEYS_REMOTE_OUTSIDE),
            UnreadableField::Available => Place::Both(lexicon::KEYS_AVAILABLE),
            UnreadableField::TargetYears => Place::Criteria(lexicon::KEYS_TARGET_YEARS),
            UnreadableField::MinSalary => Place::Criteria(lexicon::KEYS_MIN_SALARY),
            UnreadableField::PermanentPlaces => Place::Criteria(lexicon::KEYS_PERMANENT_PLACES),
            UnreadableField::PermanentRemoteMin => Place::Criteria(lexicon::KEYS_PERMANENT_REMOTE),
            UnreadableField::Focus => Place::Top(lexicon::KEYS_FOCUS),
            UnreadableField::Roles => Place::Top(lexicon::KEYS_TARGET_ROLES),
            UnreadableField::WishDayRate => Place::Preferences(lexicon::KEYS_RATE_WISH),
            UnreadableField::Remote => Place::Preferences(lexicon::KEYS_REMOTE_WISH),
            UnreadableField::Regions => Place::Preferences(lexicon::KEYS_REGIONS),
            UnreadableField::WishIndustries => Place::Preferences(lexicon::KEYS_INDUSTRIES),
        }
    }

    /// The field of a key the engine reports as not understood (`criterionNotUnderstood`);
    /// `None` for a key the form does not show.
    pub fn of_key(key: &str) -> Option<UnreadableField> {
        Self::ALL.into_iter().find(|field| {
            let (Place::Top(keys)
            | Place::Criteria(keys)
            | Place::Preferences(keys)
            | Place::Both(keys)) = field.place();
            keys.contains(&key)
        })
    }
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
                no_permanent: c.no_permanent,
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

/// The form normalized, or the first value out of range: `field` names it, `row` the entry
/// of a list of rows (competences, languages; counted in the normalized list, without its
/// empty rows).
pub(crate) fn validate(form: &ProfileForm) -> Result<ProfileForm, InvalidInput> {
    let form = form.normalized();
    let fail = |field: &str, row: Option<usize>| InvalidInput::ProfileValue {
        field: field.to_owned(),
        row: row.and_then(|r| u32::try_from(r).ok()),
    };
    let within = |value: Option<u32>, max: u32| value.is_none_or(|n| n <= max);
    let fits = |items: &[String]| {
        items.len() <= MAX_ITEMS && items.iter().all(|t| t.chars().count() <= MAX_TEXT)
    };
    let one = |text: &String| fits(std::slice::from_ref(text));
    let c = &form.criteria;
    for (value, max, field) in [
        (form.years, MAX_YEARS, "years"),
        (c.min_day_rate, MAX_DAY_RATE, "minDayRate"),
        (c.target_years, MAX_YEARS, "targetYears"),
        (c.min_salary, MAX_SALARY, "minSalary"),
        (c.permanent_remote_min, MAX_PERCENT, "permanentRemoteMin"),
        (form.wishes.day_rate, MAX_DAY_RATE, "wishDayRate"),
    ] {
        if !within(value, max) {
            return Err(fail(field, None));
        }
    }
    if form.focus.len() > MAX_FOCUS || !fits(&form.focus) {
        return Err(fail("focus", None));
    }
    for (items, field) in [
        (&form.roles, "roles"),
        (&form.wishes.regions, "regions"),
        (&form.wishes.industries, "wishIndustries"),
    ] {
        if !fits(items) {
            return Err(fail(field, None));
        }
    }
    if !one(&form.name) {
        return Err(fail("name", None));
    }
    if !one(&form.title) {
        return Err(fail("title", None));
    }
    if form.competences.len() > MAX_ITEMS {
        return Err(fail("competences", None));
    }
    for (i, row) in form.competences.iter().enumerate() {
        if !one(&row.name) || !within(row.years, MAX_YEARS) || !fits(&row.aliases) {
            return Err(fail("competences", Some(i)));
        }
    }
    for (items, field) in [
        (&form.strengths, "strengths"),
        (&form.keywords, "keywords"),
        (&form.degrees, "degrees"),
        (&form.industries, "industries"),
        (&form.tools, "tools"),
        (&form.certificates, "certificates"),
    ] {
        if !fits(items) {
            return Err(fail(field, None));
        }
    }
    if form.languages.len() > MAX_ITEMS {
        return Err(fail("languages", None));
    }
    if let Some(i) = form.languages.iter().position(|row| !one(&row.language)) {
        return Err(fail("languages", Some(i)));
    }
    if !fits(&c.countries) {
        return Err(fail("countries", None));
    }
    if !fits(&c.permanent_places) {
        return Err(fail("permanentPlaces", None));
    }
    if let ProfileAvailability::From { date } = &c.available
        && date.parse::<Date>().is_err()
    {
        return Err(fail("available", None));
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

/// The first of `keys` that `item` holds with a value (not null), and that value.
fn first<'a>(item: &'a Json, keys: &[&'static str]) -> Option<(&'static str, &'a Json)> {
    keys.iter().find_map(|key| {
        item.get(key)
            .filter(|v| **v != Json::Null)
            .map(|v| (*key, v))
    })
}

/// The key to write in `item`: the first of `keys` it holds, else the first (German) one.
fn key_in(item: &Json, keys: &[&'static str]) -> &'static str {
    keys.iter()
        .copied()
        .find(|key| item.get(key).is_some())
        .unwrap_or(keys[0])
}

/// The list the engine reads among `keys` (the first that is a list), else the first key.
fn list_key(doc: &Json, keys: &[&'static str]) -> &'static str {
    keys.iter()
        .copied()
        .find(|key| doc.get(key).and_then(Json::as_array).is_some())
        .unwrap_or_else(|| key_in(doc, keys))
}

/// The preferences section the engine reads (German or English), else the German one.
fn preferences_key(doc: &Json) -> &'static str {
    lexicon::KEY_PREFERENCES_ALIASES
        .iter()
        .copied()
        .find(|key| doc.get(key).is_some_and(Json::is_object))
        .unwrap_or(lexicon::KEY_PREFERENCES)
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

/// `sprachen[]` (`languages[]`) with `sprache` (`language`, `name`) and `niveau` (`level`).
fn read_languages(doc: &Json) -> Vec<ProfileLanguage> {
    items(doc, list_key(doc, lex::KEYS_LANGUAGES))
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            Some(ProfileLanguage {
                language: item_text(item, lex::KEYS_LANGUAGE)?.to_owned(),
                level: first(item, lex::KEYS_LEVEL)
                    .and_then(|(_, level)| level.as_str())
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

/// Texts of a list the way the engine reads Schwerpunkte, target roles and the listed
/// wishes: strings, or objects with a text under one of `KEYS_ITEM_TEXT`.
fn read_texts(list: Option<&Json>) -> Vec<String> {
    list.and_then(Json::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item_text(item, lexicon::KEYS_ITEM_TEXT))
        .map(str::to_owned)
        .collect()
}

/// The texts of the first of `keys` with a value.
fn read_texts_at(doc: &Json, keys: &[&'static str]) -> Vec<String> {
    read_texts(first(doc, keys).map(|(_, value)| value))
}

fn read_wishes(doc: &Json) -> ProfileWishes {
    let section = doc.get(preferences_key(doc));
    let at = |keys: &[&'static str]| section.and_then(|s| first(s, keys).map(|(_, v)| v));
    ProfileWishes {
        day_rate: at(lexicon::KEYS_RATE_WISH).and_then(whole),
        remote: at(lexicon::KEYS_REMOTE_WISH)
            .and_then(Json::as_str)
            .and_then(RemoteWish::read),
        regions: read_texts(at(lexicon::KEYS_REGIONS)),
        industries: read_texts(at(lexicon::KEYS_INDUSTRIES)),
    }
}

fn read_criteria(doc: &Json) -> ProfileCriteria {
    let c = matching::hard_criteria(&doc.to_value());
    ProfileCriteria {
        min_day_rate: c.min_rate.and_then(|n| u32::try_from(n).ok()),
        countries: c.countries.unwrap_or_default(),
        no_anue: c.anue_excluded,
        no_permanent: c.permanent_excluded,
        available: match c.available {
            Availability::Unset => ProfileAvailability::Unset,
            Availability::Now => ProfileAvailability::Now,
            Availability::From(day) => ProfileAvailability::From {
                date: day.to_string(),
            },
        },
        remote_outside: c.remote_outside != Some(false),
        target_years: c.target_years,
        min_salary: c.min_salary.and_then(|n| u32::try_from(n).ok()),
        permanent_places: c.places.unwrap_or_default(),
        permanent_remote_min: c.remote_min.and_then(|n| u32::try_from(n).ok()),
    }
}

/// The form of a profile document, read the way the engine reads it. Of more than
/// `MAX_FOCUS` Schwerpunkte the form takes the first ones, as the engine does.
pub(crate) fn read(doc: &Json) -> ProfileForm {
    let mut focus = read_texts_at(doc, lexicon::KEYS_FOCUS);
    focus.truncate(MAX_FOCUS);
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
        focus,
        roles: read_texts_at(doc, lexicon::KEYS_TARGET_ROLES),
        wishes: read_wishes(doc),
        criteria: read_criteria(doc),
    }
    .normalized()
}

/// More Schwerpunkte in the document than the form holds: saving writes the ones it shows.
fn focus_overflows(doc: &Json) -> bool {
    read_texts_at(doc, lexicon::KEYS_FOCUS).len() > MAX_FOCUS
}

// ------------------------------------------------------------------------------ writing

/// Writes into `doc` every field of `after` that differs from `before` (the form as the
/// editor received it), after removing the values of `clear` wherever they are. Fields in
/// canonical order, so a new profile reads like the template of the skill; a key that is
/// new in an existing profile goes to its end.
pub(crate) fn merge(
    doc: &mut Json,
    before: &ProfileForm,
    after: &ProfileForm,
    clear: &[UnreadableField],
) {
    let before = before.normalized();
    let after = after.normalized();
    if !doc.is_object() {
        *doc = Json::object();
    }
    remove_fields(doc, clear);
    if after.name != before.name {
        set_text(doc, KEY_NAME, &after.name);
    }
    if after.title != before.title {
        set_text(doc, KEY_TITLE, &after.title);
    }
    if after.roles != before.roles {
        let key = key_in(doc, lexicon::KEYS_TARGET_ROLES);
        write_free_list(doc, key, &after.roles);
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
    if after.focus != before.focus || focus_overflows(doc) {
        let key = key_in(doc, lexicon::KEYS_FOCUS);
        write_free_list(doc, key, &after.focus);
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

/// Removes every key behind the fields, in every section the engine reads it from.
fn remove_fields(doc: &mut Json, fields: &[UnreadableField]) {
    for field in fields {
        match field.place() {
            Place::Top(keys) => {
                for key in keys {
                    doc.remove(key);
                }
            }
            Place::Criteria(keys) => remove_all(doc, lexicon::KEY_CRITERIA_ALIASES, keys),
            Place::Preferences(keys) => remove_all(doc, lexicon::KEY_PREFERENCES_ALIASES, keys),
            Place::Both(keys) => {
                remove_all(doc, lexicon::KEY_CRITERIA_ALIASES, keys);
                remove_all(doc, lexicon::KEY_PREFERENCES_ALIASES, keys);
            }
        }
    }
}

/// Removes `keys` from each of the `sections`.
fn remove_all(doc: &mut Json, sections: &[&str], keys: &[&str]) {
    for section in sections {
        for key in keys {
            remove_in(doc, section, key);
        }
    }
}

/// The wishes inside the preferences section the profile uses (created only when there is
/// something to write), each under the key that is there.
fn write_wishes(doc: &mut Json, before: &ProfileWishes, after: &ProfileWishes) {
    let section = preferences_key(doc);
    let key_of =
        |doc: &Json, keys: &[&'static str]| doc.get(section).map_or(keys[0], |s| key_in(s, keys));
    if after.day_rate != before.day_rate {
        let keys = lexicon::KEYS_RATE_WISH;
        if let Some(rate) = after.day_rate {
            let key = key_of(doc, keys);
            doc.object_mut(section).set(key, Json::number(rate));
        } else {
            remove_all(doc, &[section], keys);
        }
    }
    if after.remote != before.remote {
        let keys = lexicon::KEYS_REMOTE_WISH;
        if let Some(wish) = after.remote {
            let key = key_of(doc, keys);
            doc.object_mut(section).set(key, Json::text(wish.text()));
        } else {
            remove_all(doc, &[section], keys);
        }
    }
    for (keys, old, new) in [
        (lexicon::KEYS_REGIONS, &before.regions, &after.regions),
        (
            lexicon::KEYS_INDUSTRIES,
            &before.industries,
            &after.industries,
        ),
    ] {
        if new == old {
            continue;
        }
        if new.is_empty() {
            remove_all(doc, &[section], keys);
        } else {
            let key = key_of(doc, keys);
            // Plain texts, whatever the same key means at the top level.
            write_free_list(doc.object_mut(section), key, new);
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
fn write_number(item: &mut Json, keys: &[&'static str], value: Option<u32>) {
    match value {
        Some(n) => {
            let key = key_in(item, keys);
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
fn write_texts(item: &mut Json, keys: &[&'static str], texts: &[String]) {
    let key = key_in(item, keys);
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

/// A list of plain texts (Schwerpunkte, target roles, listed wishes); a list that holds
/// objects keeps them and gets new ones under the key it uses.
fn write_free_list(doc: &mut Json, key: &str, texts: &[String]) {
    let objects = items(doc, key).iter().any(Json::is_object);
    let subs: &[&str] = if objects {
        lexicon::KEYS_ITEM_TEXT
    } else {
        &[]
    };
    write_texts_list(doc, key, subs, texts);
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
    list: (&str, &[&str]),
    rows: (&[R], &[R]),
    origin: impl Fn(&R) -> Option<u32>,
    update: impl Fn(&Json, Option<&R>, &R) -> Json,
) {
    let (key, main) = list;
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
            .filter(|(i, item)| !used[*i] && item_text(item, main).is_none())
            .map(|(_, item)| item.clone()),
    );
    put_list(doc, key, out);
}

fn write_competences(doc: &mut Json, before: &[ProfileCompetence], after: &[ProfileCompetence]) {
    write_rows(
        doc,
        (KEY_COMPETENCES, &[KEY_COMPETENCE]),
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

/// The item keys a language list uses (the first object's), else the German ones.
fn language_keys(list: &[Json]) -> (&'static str, &'static str) {
    let sample = list.iter().find(|item| item.is_object());
    let pick = |keys: &[&'static str]| sample.map_or(keys[0], |item| key_in(item, keys));
    (pick(lex::KEYS_LANGUAGE), pick(lex::KEYS_LEVEL))
}

fn write_languages(doc: &mut Json, before: &[ProfileLanguage], after: &[ProfileLanguage]) {
    let key = list_key(doc, lex::KEYS_LANGUAGES);
    let (name_key, level_key) = language_keys(items(doc, key));
    let level_text = |level: LanguageLevel| {
        if level == LanguageLevel::Native && level_key != lex::KEY_LEVEL {
            LEVEL_NATIVE_EN
        } else {
            level.text()
        }
    };
    write_rows(
        doc,
        (key, lex::KEYS_LANGUAGE),
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
                let at = key_in(&item, lex::KEYS_LANGUAGE);
                let at = if item.get(at).is_some() { at } else { name_key };
                item.set(at, Json::text(&row.language));
            }
            if prior.map(|p| p.level) != Some(row.level) {
                let at = key_in(&item, lex::KEYS_LEVEL);
                let at = if item.get(at).is_some() {
                    at
                } else {
                    level_key
                };
                match row.level {
                    Some(level) => item.set(at, Json::text(level_text(level))),
                    None => {
                        for key in lex::KEYS_LEVEL {
                            item.remove(key);
                        }
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

/// Where a criterion is (German or English section and key, with a value), else
/// `harte_kriterien` with the German key.
fn criterion_at(doc: &Json, keys: &[&'static str]) -> (&'static str, &'static str) {
    lexicon::KEY_CRITERIA_ALIASES
        .iter()
        .find_map(|section| {
            let values = doc.get(section)?;
            first(values, keys).map(|(key, _)| (*section, key))
        })
        .unwrap_or((lexicon::KEY_CRITERIA, keys[0]))
}

/// A criterion where it is (see [`criterion_at`]); `None` removes it everywhere.
fn write_criterion(doc: &mut Json, keys: &[&'static str], value: Option<Json>) {
    match value {
        Some(value) => {
            let (section, key) = criterion_at(doc, keys);
            doc.object_mut(section).set(key, value);
        }
        None => remove_all(doc, lexicon::KEY_CRITERIA_ALIASES, keys),
    }
}

/// The availability where the engine reads it (the criteria, else the preferences, German
/// and English), else `harte_kriterien.verfuegbar_ab`; `None` removes it everywhere.
fn write_available(doc: &mut Json, text: Option<String>) {
    let keys = lexicon::KEYS_AVAILABLE;
    let Some(text) = text else {
        remove_all(doc, lexicon::KEY_CRITERIA_ALIASES, keys);
        remove_all(doc, lexicon::KEY_PREFERENCES_ALIASES, keys);
        return;
    };
    let non_empty = |value: &Json| value.as_str().is_some_and(|t| !t.trim().is_empty());
    let found = lexicon::KEY_CRITERIA_ALIASES
        .iter()
        .chain(lexicon::KEY_PREFERENCES_ALIASES)
        .find_map(|section| {
            let values = doc.get(section)?;
            keys.iter()
                .find(|key| values.get(key).is_some_and(non_empty))
                .map(|key| (*section, *key))
        });
    let (section, key) = found.unwrap_or((lexicon::KEY_CRITERIA, keys[0]));
    doc.object_mut(section).set(key, Json::String(text));
}

/// A profile value excludes ANÜ, as the engine reads it.
fn names_anue(item: &Json) -> bool {
    facts::excludes_anue(&item.to_value()) == Some(true)
}

/// A profile value excludes permanent employment, as the engine reads it.
fn names_permanent(item: &Json) -> bool {
    facts::excludes_permanent(&item.to_value())
}

/// The excluded contract types where they are: switching one on adds its value, switching
/// it off removes every entry that names it; other entries stay.
fn write_contracts(doc: &mut Json, before: &ProfileCriteria, after: &ProfileCriteria) {
    if after.no_anue == before.no_anue && after.no_permanent == before.no_permanent {
        return;
    }
    let (section, key) = criterion_at(doc, lexicon::KEYS_EXCLUDED_CONTRACTS);
    let mut kinds: Vec<Json> = match doc.get(section).and_then(|s| s.get(key)) {
        Some(Json::Array(list)) => list.clone(),
        Some(text @ Json::String(_)) => vec![text.clone()],
        _ => Vec::new(),
    };
    for (on, was, names, value) in [
        (
            after.no_anue,
            before.no_anue,
            names_anue as fn(&Json) -> bool,
            lexicon::CONTRACT_ANUE,
        ),
        (
            after.no_permanent,
            before.no_permanent,
            names_permanent,
            CONTRACT_PERMANENT,
        ),
    ] {
        if on == was {
            continue;
        }
        if on {
            if !kinds.iter().any(names) {
                kinds.push(Json::text(value));
            }
        } else {
            kinds.retain(|kind| !names(kind));
        }
    }
    if kinds.is_empty() {
        remove_in(doc, section, key);
    } else {
        doc.object_mut(section).set(key, Json::Array(kinds));
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
    if after.min_day_rate != before.min_day_rate {
        write_criterion(
            doc,
            lexicon::KEYS_MIN_RATE,
            after.min_day_rate.map(Json::number),
        );
        if after.min_day_rate.is_none() {
            remove_all(
                doc,
                lexicon::KEY_PREFERENCES_ALIASES,
                &[lexicon::KEY_RATE_FROM],
            );
        }
    }
    if after.countries != before.countries {
        let countries = (!after.countries.is_empty()).then(|| Json::texts(&after.countries));
        write_criterion(doc, lexicon::KEYS_COUNTRIES, countries);
    }
    write_contracts(doc, before, after);
    if after.available != before.available {
        write_available(
            doc,
            match &after.available {
                ProfileAvailability::Unset => None,
                ProfileAvailability::Now => Some(lexicon::AVAILABLE_NOW.to_owned()),
                ProfileAvailability::From { date } => Some(german_date(date)),
            },
        );
    }
    if after.remote_outside != before.remote_outside {
        write_criterion(
            doc,
            lexicon::KEYS_REMOTE_OUTSIDE,
            Some(Json::Bool(after.remote_outside)),
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::matching::{ProfileWarningCode, compile_profile};

    fn doc(value: &Value) -> Json {
        serde_json::from_str(&value.to_string()).unwrap()
    }

    /// Without the origins of the rows (a form read back from a written file has new ones).
    fn plain(form: &ProfileForm) -> ProfileForm {
        let mut form = form.normalized();
        for row in &mut form.competences {
            row.origin = None;
        }
        for row in &mut form.languages {
            row.origin = None;
        }
        form
    }

    fn texts(items: &[&str]) -> Vec<String> {
        items.iter().map(|t| (*t).to_owned()).collect()
    }

    /// A profile under English keys, as the engine reads it too.
    fn english_profile() -> Json {
        doc(&json!({
            "kernkompetenzen": [{"kompetenz": "Controlling"}, {"kompetenz": "Treasury"}],
            "focus_areas": ["Controlling"],
            "target_roles": ["Interim CFO"],
            "preferences": {
                "desired_day_rate": 1100,
                "remote": "hybrid",
                "regions": ["Hamburg"],
                "industries": ["Chemie"]
            },
            "languages": [{"language": "English", "level": "C1"}],
            "hard_criteria": {
                "min_day_rate": 900,
                "countries": ["DE"],
                "excluded_contract_types": ["anue"],
                "remote_outside_allowed": false,
                "available_from": "2026-11-01"
            }
        }))
    }

    /// A profile under English keys fills the form.
    #[test]
    fn english_keys_fill_the_form() {
        let before = read(&english_profile());
        assert_eq!(before.focus, ["Controlling"]);
        assert_eq!(before.roles, ["Interim CFO"]);
        assert_eq!(
            before.wishes,
            ProfileWishes {
                day_rate: Some(1100),
                remote: Some(RemoteWish::Partly),
                regions: texts(&["Hamburg"]),
                industries: texts(&["Chemie"]),
            }
        );
        assert_eq!(before.languages[0].language, "English");
        assert_eq!(before.languages[0].level, Some(LanguageLevel::C1));
        let c = &before.criteria;
        assert_eq!(
            (c.min_day_rate, c.countries.as_slice()),
            (Some(900), &texts(&["DE"])[..])
        );
        assert!(c.no_anue && !c.no_permanent && !c.remote_outside);
        assert_eq!(
            c.available,
            ProfileAvailability::From {
                date: "2026-11-01".into()
            }
        );
    }

    /// Every change of a profile under English keys goes back under the key that is there:
    /// no German key appears next to an English one.
    #[test]
    fn english_keys_are_written_where_they_are() {
        let mut profile = english_profile();
        let before = read(&profile);
        let mut after = before.clone();
        after.focus.push("Treasury".into());
        after.roles.push("Head of Controlling".into());
        after.wishes = ProfileWishes {
            day_rate: Some(1200),
            remote: Some(RemoteWish::Full),
            regions: texts(&["Hamburg", "Berlin"]),
            industries: texts(&["Chemie", "Pharma"]),
        };
        after.languages[0].level = Some(LanguageLevel::C2);
        after.languages.push(ProfileLanguage {
            language: "German".into(),
            level: Some(LanguageLevel::Native),
            origin: None,
        });
        after.criteria.min_day_rate = Some(1000);
        after.criteria.countries.push("AT".into());
        after.criteria.no_anue = false;
        after.criteria.no_permanent = true;
        after.criteria.remote_outside = true;
        after.criteria.available = ProfileAvailability::Now;
        merge(&mut profile, &before, &after, &[]);

        let value = profile.to_value();
        for german in [
            "schwerpunkte",
            "wunschrollen",
            "einsatzpraeferenzen",
            "sprachen",
            "harte_kriterien",
        ] {
            assert!(value.get(german).is_none(), "{german}: {value}");
        }
        assert_eq!(value["focus_areas"], json!(["Controlling", "Treasury"]));
        assert_eq!(
            value["target_roles"],
            json!(["Interim CFO", "Head of Controlling"])
        );
        assert_eq!(
            value["preferences"],
            json!({
                "desired_day_rate": 1200,
                "remote": "voll",
                "regions": ["Hamburg", "Berlin"],
                "industries": ["Chemie", "Pharma"]
            })
        );
        assert_eq!(
            value["languages"],
            json!([
                {"language": "English", "level": "C2"},
                {"language": "German", "level": "native"}
            ])
        );
        assert_eq!(
            value["hard_criteria"],
            json!({
                "min_day_rate": 1000,
                "countries": ["DE", "AT"],
                "excluded_contract_types": ["festanstellung"],
                "remote_outside_allowed": true,
                "available_from": "sofort"
            })
        );
        assert_eq!(plain(&read(&profile)), plain(&after));
    }

    /// Every value the engine cannot read names a field of the form; removing the field's
    /// value takes its keys out wherever they are, and nothing else.
    #[test]
    fn a_value_that_does_not_read_can_be_removed() {
        let mut profile = doc(&json!({
            "name": "Erika Beispiel",
            "kernkompetenzen": [{"kompetenz": "Controlling"}],
            "schwerpunkte": 7,
            "target_roles": "CFO",
            "harte_kriterien": {
                "min_tagessatz": "viel",
                "laender": "Deutschland",
                "ausgeschlossene_vertragsarten": 5,
                "remote_ausserhalb_erlaubt": "vielleicht",
                "zielprofil_min_jahre": "senior",
                "festanstellung_orte": [],
                "festanstellung_remote_min": "viel"
            },
            "hard_criteria": {"min_salary": "hoch"},
            "einsatzpraeferenzen": {
                "verfuegbar_ab": "bald",
                "tagessatz_wunsch": "hoch",
                "remote": "egal",
                "regionen": 5,
                "branchen": {}
            }
        }));
        let warnings = compile_profile(&profile.to_value())
            .summary()
            .warnings
            .clone();
        let mut fields: Vec<UnreadableField> = warnings
            .iter()
            .filter_map(|w| match w.code {
                ProfileWarningCode::AvailabilityNotUnderstood => Some(UnreadableField::Available),
                ProfileWarningCode::CriterionNotUnderstood => {
                    let key = w.params["key"].as_str().unwrap();
                    Some(UnreadableField::of_key(key).unwrap_or_else(|| panic!("{key}")))
                }
                _ => None,
            })
            .collect();
        fields.sort_by_key(|f| format!("{f:?}"));
        fields.dedup();
        assert_eq!(fields.len(), UnreadableField::ALL.len(), "{warnings:?}");

        let form = read(&profile);
        merge(&mut profile, &form, &form, &fields);
        let value = profile.to_value();
        assert_eq!(
            value,
            json!({
                "name": "Erika Beispiel",
                "kernkompetenzen": [{"kompetenz": "Controlling"}],
                "harte_kriterien": {},
                "hard_criteria": {},
                "einsatzpraeferenzen": {}
            })
        );
        let left = compile_profile(&value).summary().warnings.clone();
        assert!(
            !left.iter().any(|w| matches!(
                w.code,
                ProfileWarningCode::CriterionNotUnderstood
                    | ProfileWarningCode::AvailabilityNotUnderstood
            )),
            "{left:?}"
        );
        assert_eq!(UnreadableField::of_key("hobbys"), None);
    }

    /// Of more than five Schwerpunkte the form takes the first five, as the engine does, and
    /// saving writes those five (the file then says what the form showed).
    #[test]
    fn more_than_five_focus_are_trimmed_and_saved() {
        let names = ["A1", "B2", "C3", "D4", "E5", "F6", "G7"].map(|n| format!("Kompetenz {n}"));
        let mut profile = doc(&json!({
            "kernkompetenzen": names.iter().map(|n| json!({"kompetenz": n})).collect::<Vec<_>>(),
            "schwerpunkte": names,
        }));
        let codes = |profile: &Json| -> Vec<ProfileWarningCode> {
            compile_profile(&profile.to_value())
                .summary()
                .warnings
                .iter()
                .map(|w| w.code)
                .collect()
        };
        assert!(codes(&profile).contains(&ProfileWarningCode::FocusTrimmed));
        let form = read(&profile);
        assert_eq!(form.focus, names[..MAX_FOCUS]);
        assert!(validate(&form).is_ok(), "the trimmed form saves");
        merge(&mut profile, &form, &form, &[]);
        assert_eq!(
            profile.to_value()["schwerpunkte"],
            json!(names[..MAX_FOCUS])
        );
        assert!(!codes(&profile).contains(&ProfileWarningCode::FocusTrimmed));
        // Five or fewer: an unchanged form writes nothing.
        let same = profile.clone();
        merge(&mut profile, &form, &form, &[]);
        assert_eq!(profile, same);
    }

    /// A value out of range names its field, and in a list of rows the row (counted without
    /// the empty rows the form drops).
    #[test]
    fn validation_names_the_field_and_the_row() {
        let error = |form: &ProfileForm| match validate(form) {
            Err(InvalidInput::ProfileValue { field, row }) => (field, row),
            other => panic!("{other:?}"),
        };
        let row = |name: &str, years: Option<u32>| ProfileCompetence {
            name: name.into(),
            years,
            aliases: Vec::new(),
            origin: None,
        };
        let mut form = ProfileForm {
            competences: vec![
                row("", None),
                row("Controlling", Some(12)),
                row("IFRS", Some(71)),
            ],
            ..ProfileForm::default()
        };
        assert_eq!(error(&form), ("competences".to_owned(), Some(1)));
        form.competences[2].years = Some(8);
        form.competences[1].aliases = vec!["x".repeat(MAX_TEXT + 1)];
        assert_eq!(error(&form), ("competences".to_owned(), Some(0)));
        form.competences[1].aliases.clear();
        form.languages = vec![
            ProfileLanguage {
                language: "Englisch".into(),
                level: None,
                origin: None,
            },
            ProfileLanguage {
                language: "y".repeat(MAX_TEXT + 1),
                level: None,
                origin: None,
            },
        ];
        assert_eq!(error(&form), ("languages".to_owned(), Some(1)));
        form.languages.pop();
        form.criteria.min_day_rate = Some(MAX_DAY_RATE + 1);
        assert_eq!(error(&form), ("minDayRate".to_owned(), None));
        form.criteria.min_day_rate = Some(950);
        assert!(validate(&form).is_ok());
    }

    /// The remote switch reads a missing key as allowed, like the engine; only a change
    /// writes it, where it is.
    #[test]
    fn remote_abroad_is_allowed_unless_the_profile_says_no() {
        let empty = doc(&json!({}));
        assert!(read(&empty).criteria.remote_outside);
        assert!(ProfileForm::default().criteria.remote_outside);
        let no = doc(&json!({"harte_kriterien": {"remote_ausserhalb_erlaubt": false}}));
        assert!(!read(&no).criteria.remote_outside);

        let mut profile = empty.clone();
        let form = read(&profile);
        merge(&mut profile, &form, &form, &[]);
        assert_eq!(profile, empty, "unchanged: nothing written");
        let mut off = form.clone();
        off.criteria.remote_outside = false;
        merge(&mut profile, &form, &off, &[]);
        assert_eq!(
            profile.to_value(),
            json!({"harte_kriterien": {"remote_ausserhalb_erlaubt": false}})
        );
    }

    /// The two excluded contract types share one list: each switch adds or removes only its
    /// own entries, also when the list is a single text.
    #[test]
    fn contract_switches_share_one_list() {
        let mut profile =
            doc(&json!({"harte_kriterien": {"ausgeschlossene_vertragsarten": "ANÜ"}}));
        let before = read(&profile);
        assert!(before.criteria.no_anue && !before.criteria.no_permanent);
        let mut after = before.clone();
        after.criteria.no_permanent = true;
        merge(&mut profile, &before, &after, &[]);
        assert_eq!(
            profile.to_value()["harte_kriterien"]["ausgeschlossene_vertragsarten"],
            json!(["ANÜ", "festanstellung"])
        );
        let before = read(&profile);
        let mut after = before.clone();
        after.criteria.no_anue = false;
        merge(&mut profile, &before, &after, &[]);
        assert_eq!(
            profile.to_value()["harte_kriterien"]["ausgeschlossene_vertragsarten"],
            json!(["festanstellung"])
        );
        let read_back = read(&profile).criteria;
        assert!(!read_back.no_anue && read_back.no_permanent);
        let before = read(&profile);
        let mut none = before.clone();
        none.criteria.no_permanent = false;
        merge(&mut profile, &before, &none, &[]);
        assert_eq!(profile.to_value(), json!({"harte_kriterien": {}}));
    }
}
