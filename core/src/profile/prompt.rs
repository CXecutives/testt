//! The prompt a user hands to an AI together with a CV ("Aus Lebenslauf erstellen" and "Aus
//! Lebenslauf aktualisieren" in the Profil view): the AI answers with a profile in exactly the
//! JSON the editor reads, and the user pastes the answer back ([`super::answer`] reads it).
//! Like the app's other prompts it addresses the assistant as "du" without naming a product,
//! in the app's language; the JSON keys and the fixed values stay German in both (they are the
//! profile format).
//!
//! The rules follow the engine: short terms (a sentence proves no requirement), only true
//! synonyms in `auch` (an alias counts as the competence), years from the CV's dates (the text
//! says what day it is), Schwerpunkte that are competences, target roles with a field, and
//! wishes and exclusion criteria only where the CV or the user states them (a wrong one costs
//! points or excludes good jobs). An update carries the CV part of the stored profile (no
//! name, wishes or criteria), so an entry that means the same keeps its spelling and the
//! editor's merge finds it; its answer brings no wishes and criteria, the app keeps the
//! user's own.
//!
//! external contract - do not translate: the German text and the profile JSON keys it names
//! (the tests check that every key of the skeleton is explained and that the editor reads it).

use jiff::civil::Date;

use super::answer;
use super::form::ProfileForm;
use super::json::Json;
use crate::settings::Language;

/// The keys a CV fills, in the order the app writes them, the career stations (read by the
/// engine, kept in the file) last.
const SKELETON_CV: &str = r#"{
  "name": "",
  "titel": "",
  "wunschrollen": [],
  "berufserfahrung_jahre": null,
  "ausbildung": [
    { "abschluss": "" }
  ],
  "kernkompetenzen": [
    { "kompetenz": "", "jahre": null, "auch": [] }
  ],
  "schwerpunkte": [],
  "methoden_tools": [
    { "name": "" }
  ],
  "zertifizierungen": [
    { "name": "" }
  ],
  "branchen": [
    { "branche": "" }
  ],
  "sprachen": [
    { "sprache": "", "niveau": "" }
  ],
  "alleinstellungsmerkmale": [],
  "keywords": [],
  "stationen": [
    { "zeitraum": "", "rolle": "", "schwerpunkte": [] }
  ]"#;

/// The user's settings after them: only a new profile asks for them.
const SKELETON_SETTINGS: &str = r#",
  "einsatzpraeferenzen": {
    "tagessatz_wunsch": null,
    "remote": "",
    "regionen": [],
    "branchen": []
  },
  "harte_kriterien": {
    "min_tagessatz": null,
    "laender": [],
    "ausgeschlossene_vertragsarten": [],
    "verfuegbar_ab": ""
  }"#;

/// The skeleton the AI fills, with or without the user's settings.
pub(crate) fn skeleton(settings: bool) -> String {
    let tail = if settings { SKELETON_SETTINGS } else { "" };
    format!("{SKELETON_CV}{tail}\n}}")
}

const INTRO: &str = "Du unterstützt mich als KI-Assistent bei meinem Beraterprofil. Bitte erstelle aus meinem angehängten Lebenslauf das Profil für meine Job-Alert-App. Die App vergleicht jede Stellenanzeige Begriff für Begriff mit diesem Profil; je genauer es den Lebenslauf wiedergibt, desto besser findet sie passende Projekte.";

const INTRO_UPDATE: &str = "Du unterstützt mich als KI-Assistent bei meinem Beraterprofil. Bitte aktualisiere das Profil meiner Job-Alert-App mit meinem angehängten Lebenslauf; mein bisheriges Profil steht unten. Die App vergleicht jede Stellenanzeige Begriff für Begriff mit diesem Profil; je genauer es den Lebenslauf wiedergibt, desto besser findet sie passende Projekte.";

/// `{today}` is replaced with the day the app writes the prompt.
const PRINCIPLES: &str = "Grundsätze
1. Der Lebenslauf ist die einzige Quelle. Übernimm nur, was dort steht oder direkt daraus folgt; erfinde, schätze und ergänze nichts.
2. Was der Lebenslauf nicht hergibt, bleibt leer, Text als \"\", Listen als [] und Zahlen als null. Leere Felder fülle ich selbst in der App.
3. Jahre rechnest du aus den Daten des Lebenslaufs in ganzen Jahren, abgerundet; Zeiträume, die sich überschneiden, zählen einmal. Heute ist der {today}.
4. Schreib Begriffe kurz und so, wie Stellenanzeigen sie schreiben; Sprachen, Branchen und Sätze auf Deutsch.
5. Keine Kontaktdaten, keine Adresse, kein Geburtsdatum, kein Familienstand und keine Namen von Arbeitgebern oder Kunden.";

const UPDATE: &str = "Die Aktualisierung
1. Erstelle das Profil vollständig neu aus dem Lebenslauf. Was nur im bisherigen Profil steht, lässt du weg; die App behält es.
2. Meint ein Eintrag dasselbe wie einer im bisherigen Profil, schreib ihn genau so wie dort, damit die App beide zusammenführt.
3. Rechne alle Jahre neu aus dem Lebenslauf.
4. Meine Wünsche und Ausschlusskriterien pflege ich in der App; sie gehören nicht in die Antwort.";

const FIELDS: &str = "Die Felder
- name ist mein Vor- und Nachname.
- titel ist meine berufliche Rolle in zwei bis fünf Wörtern, wie sie über einer passenden Anzeige stehen könnte, etwa Interim CFO, IT-Programmmanager oder Entwicklungsingenieur Elektronik. Kein Abschluss und kein Satz.
- wunschrollen sind zwei bis sechs Rollen, für die ich laut Lebenslauf gebucht werden will, jede mit ihrem Fachgebiet, etwa Head of Controlling oder Projektleiter Inbetriebnahme. Interim Manager, Berater oder Freelancer allein nennen kein Fachgebiet. Nennen Anzeigen eine Rolle deutsch und englisch, nimm beide Formen auf.
- berufserfahrung_jahre zählt vom Beginn der ersten beruflichen Station bis heute, ohne Ausbildung, Studium, Praktika und Lücken. Nennt der Lebenslauf die Zahl selbst, gilt sie.
- ausbildung enthält je Abschluss ein Objekt. abschluss nennt die Art ausgeschrieben (Bachelor, Master, Diplom, Magister, Staatsexamen, Promotion, MBA oder eine Berufsausbildung) und das Fach, dazu (FH), (Univ.) oder (BA), wenn der Lebenslauf das sagt, etwa Diplom-Ingenieur (FH) Maschinenbau, Master of Science Wirtschaftsinformatik oder Industriekaufmann (IHK). Keine Schulabschlüsse.
- kernkompetenzen sind zehn bis zwanzig fachliche Kompetenzen, soweit der Lebenslauf sie belegt, die wichtigsten zuerst, je ein Objekt mit kompetenz, jahre und auch. kompetenz ist ein Begriff aus einem bis vier Wörtern, etwa Konzerncontrolling, Anforderungsmanagement oder Konstruktion; nichts so Allgemeines wie Management, keine Werkzeuge wie SAP (die gehören zu methoden_tools) und keine Sätze.
- jahre einer Kompetenz sind die Jahre der Stationen, in denen der Lebenslauf sie nennt; ohne solche Stationen bleibt jahre null.
- auch sind andere Begriffe, unter denen Anzeigen genau diese Kompetenz suchen, vor allem die englische oder deutsche Entsprechung, etwa Requirements Engineering zu Anforderungsmanagement. Keine Ober- oder Unterbegriffe und keine verwandten Themen, denn die App wertet jeden Begriff in auch wie die Kompetenz selbst.
- schwerpunkte sind drei bis fünf der kernkompetenzen, für die ich vor allem gebucht werden will, erkennbar an Profiltext und jüngsten Stationen. Schreib jeden Schwerpunkt genau so wie in kernkompetenzen.
- methoden_tools sind Software, Systeme, Programmiersprachen und Methoden, je ein Objekt mit name, so genau wie im Lebenslauf, etwa SAP S/4HANA FI, Power BI, Python, Scrum oder FMEA.
- zertifizierungen sind erworbene Zertifikate und Zulassungen unter ihrem gängigen Namen, je ein Objekt mit name, etwa PMP, PRINCE2 Practitioner oder ITIL 4 Foundation. Keine Schulungen ohne Abschluss.
- branchen sind die Branchen meiner Stationen, je ein Objekt mit branche, etwa Automobilindustrie, Banken oder Pharma. Keine Funktionen wie Controlling und keine Firmennamen.
- sprachen enthält jede Sprache mit sprache und niveau. niveau ist \"A1\", \"A2\", \"B1\", \"B2\", \"C1\", \"C2\" oder \"Muttersprache\"; verhandlungssicher, fließend und sehr gut werden \"C1\", gut wird \"B2\", Grundkenntnisse werden \"A2\". Ohne Angabe im Lebenslauf bleibt niveau leer.
- alleinstellungsmerkmale sind bis zu fünf kurze Sätze darüber, was mich laut Lebenslauf von anderen abhebt, jeder mit einem Beleg von dort, etwa „Leitete drei Werksanläufe bis zum Serienstart.“ Zahlen nur, wenn sie im Lebenslauf stehen, und keine Floskeln wie teamfähig oder motiviert.
- keywords sind fünf bis zwanzig weitere Fachbegriffe aus dem Lebenslauf, die in passenden Anzeigen stehen und oben noch fehlen, etwa Normen, Regelwerke und Verfahren wie IFRS 16, ISO 26262 oder GMP.
- stationen enthält jede berufliche Station als Objekt, die jüngste zuerst. zeitraum schreibst du wie 03/2021 bis 06/2024 oder 03/2021 bis heute, rolle als Funktion ohne Firmennamen und schwerpunkte als zwei bis sechs fachliche Themen, die der Lebenslauf für die Station nennt, kurz wie Kompetenzen.";

const SETTINGS: &str = "Wünsche und Ausschlusskriterien
Die Felder unter einsatzpraeferenzen und harte_kriterien füllst du nur, wenn der Lebenslauf oder ich im Chat sie ausdrücklich nennen. Sonst bleiben sie leer, und ich setze sie in der App. Tagessätze sind Euro pro Tag, ein Stundensatz zählt mal acht.
- tagessatz_wunsch ist mein Tagessatz. Ein genannter Tagessatz ist ein Wunsch, keine Untergrenze.
- remote ist \"voll\" (nur remote), \"ueberwiegend\" (mehr als die Hälfte), \"teilweise\" (bis zur Hälfte) oder \"vor_ort\" (kein Remote).
- regionen sind Orte oder Regionen, in denen ich arbeiten will, etwa Hamburg oder Rhein-Main.
- branchen unter einsatzpraeferenzen sind Branchen, in denen ich künftig arbeiten will, nicht einfach die bisherigen.
- min_tagessatz ist ein Tagessatz, unter dem ich ausdrücklich nicht arbeite.
- laender sind Ländercodes wie \"DE\", \"AT\" und \"CH\", wenn ich Einsätze auf diese Länder beschränke.
- ausgeschlossene_vertragsarten nennt \"anue\", wenn ich Arbeitnehmerüberlassung ausschließe, und \"festanstellung\", wenn ich keine Festanstellung will.
- verfuegbar_ab ist \"sofort\" oder ein Datum wie \"01.11.2026\"; ein Monat ohne Tag wird zum Ersten des Monats.";

const ANSWER: &str = "Die Antwort
Antworte nur mit dem JSON in einem einzigen Codeblock, ohne Text davor oder danach. Behalte jeden Schlüssel des Aufbaus unten, seine Schreibweise und die Reihenfolge. Ein Objekt in einer Liste zeigt den Aufbau eines Eintrags; wiederhole es für jeden Eintrag. Zahlen stehen ohne Anführungszeichen und ohne Einheit. Das JSON muss gültig sein, mit geraden doppelten Anführungszeichen, ohne Kommentare und ohne Komma vor einer schließenden Klammer.";

const CHECK: &str = "Prüfe vor dem Antworten";
const CHECKS: [&str; 4] = [
    "Steht jeder Wert im Lebenslauf oder folgt direkt aus ihm?",
    "Ist jede Kompetenz ein kurzer Begriff, und meint jeder Begriff in auch genau diese Kompetenz?",
    "Steht jeder Schwerpunkt genau so in kernkompetenzen, und nennt jede Wunschrolle ein Fachgebiet?",
    "Stammen alle Jahre aus den Daten des Lebenslaufs?",
];
const CHECK_SETTINGS: &str =
    "Sind Wünsche und Ausschlusskriterien leer, wo sie niemand genannt hat?";
const CHECK_UPDATE: &str =
    "Steht jeder Eintrag, der einem im bisherigen Profil entspricht, genau so wie dort?";
const CHECK_ANSWER: &str =
    "Ist die Antwort ein einziger gültiger JSON-Codeblock mit genau den Schlüsseln des Aufbaus?";

const CURRENT: &str = "Mein bisheriges Profil (JSON, ohne Name, Wünsche und Ausschlusskriterien)";
const STRUCTURE: &str = "Der Aufbau";

const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

/// The same prompt in English (the keys and the fixed values it names stay German).
mod en {
    pub(super) const INTRO: &str = "You support me as an AI assistant with my consultant profile. Please create the profile for my job alert app from my attached CV. The app compares every job ad with this profile term by term; the more precisely the profile reflects the CV, the better the app finds matching projects.";

    pub(super) const INTRO_UPDATE: &str = "You support me as an AI assistant with my consultant profile. Please update the profile of my job alert app with my attached CV; my current profile is below. The app compares every job ad with this profile term by term; the more precisely the profile reflects the CV, the better the app finds matching projects.";

    pub(super) const PRINCIPLES: &str = "Principles
1. The CV is the only source. Take only what it says or what follows directly from it; invent, estimate and add nothing.
2. Whatever the CV does not give stays empty, text as \"\", lists as [] and numbers as null. I fill empty fields in the app myself.
3. Work out years from the dates in the CV in whole years, rounded down; periods that overlap count once. Today is {today}.
4. Keep terms short and write them the way job ads do; languages, industries and sentences in English.
5. No contact details, no address, no date of birth, no marital status and no names of employers or clients.";

    pub(super) const UPDATE: &str = "The update
1. Build the profile anew from the CV. Leave out what only the current profile holds; the app keeps it.
2. Where an entry means the same as one in the current profile, write it exactly as there, so the app merges the two.
3. Work out all years anew from the CV.
4. I keep my preferences and exclusion criteria in the app; they do not belong in the answer.";

    pub(super) const FIELDS: &str = "The fields
- name is my first and last name.
- titel is my professional role in two to five words, as it could head a matching ad, such as Interim CFO, IT Programme Manager or Electronics Development Engineer. No degree and no sentence.
- wunschrollen are two to six roles the CV says I want to be booked for, each with its field, such as Head of Controlling or Commissioning Project Manager. Interim Manager, Consultant or Freelancer alone name no field. If ads name a role in German and in English, include both forms.
- berufserfahrung_jahre counts from the start of my first professional position until today, without vocational training, studies, internships and gaps. If the CV states the number itself, use it.
- ausbildung holds one object per degree. abschluss names the type in full (Bachelor, Master, Diplom, Magister, Staatsexamen, PhD, MBA or a vocational qualification) and the subject, with (FH), (Univ.) or (BA) if the CV says so, such as Diplom-Ingenieur (FH) Maschinenbau, Master of Science in Business Informatics or Industriekaufmann (IHK). No school leaving certificates.
- kernkompetenzen are ten to twenty skills, as far as the CV bears them out, the most important first, one object each with kompetenz, jahre and auch. kompetenz is a term of one to four words, such as Group Controlling, Requirements Engineering or Mechanical Design; nothing as general as Management, no tools such as SAP (they belong in methoden_tools) and no sentences.
- jahre of a skill are the years of the positions in which the CV names it; without such positions jahre stays null.
- auch are other terms under which ads look for exactly this skill, above all the German or English equivalent, such as Anforderungsmanagement for Requirements Engineering. No broader or narrower terms and no related topics, as the app counts every term in auch as the skill itself.
- schwerpunkte are three to five of the kernkompetenzen I most want to be booked for, as the profile summary and the latest positions show. Write each focus area exactly as in kernkompetenzen.
- methoden_tools are software, systems, programming languages and methods, one object with name each, as precise as in the CV, such as SAP S/4HANA FI, Power BI, Python, Scrum or FMEA.
- zertifizierungen are certificates and licences I hold, under their common name, one object with name each, such as PMP, PRINCE2 Practitioner or ITIL 4 Foundation. No training courses without a certificate.
- branchen are the industries of my positions, one object with branche each, such as Automotive, Banking or Pharmaceuticals. No functions such as Controlling and no company names.
- sprachen holds every language with sprache and niveau. niveau is \"A1\", \"A2\", \"B1\", \"B2\", \"C1\", \"C2\" or \"Muttersprache\" for a native language; business fluent, fluent and very good become \"C1\", good becomes \"B2\", basic knowledge becomes \"A2\". Without a level in the CV, niveau stays empty.
- alleinstellungsmerkmale are up to five short sentences on what sets me apart according to the CV, each with evidence from it, such as “Led three plant start-ups through to series production.” Numbers only if the CV states them, and no empty phrases such as team player or highly motivated.
- keywords are five to twenty further specialist terms from the CV that appear in matching ads and are still missing above, such as standards, regulations and procedures like IFRS 16, ISO 26262 or GMP.
- stationen holds every professional position as an object, the latest first. Write zeitraum like 03/2021 to 06/2024 or 03/2021 to today, rolle as the function without company names and schwerpunkte as two to six specialist topics the CV names for the position, short like skills.";

    pub(super) const SETTINGS: &str = "Preferences and exclusion criteria
Fill the fields under einsatzpraeferenzen and harte_kriterien only if the CV or I in the chat state them explicitly. Otherwise they stay empty, and I set them in the app. Day rates are euros per day; an hourly rate counts eight times.
- tagessatz_wunsch is my day rate. A day rate that is stated is a preference, not a minimum.
- remote is \"voll\" (fully remote), \"ueberwiegend\" (more than half), \"teilweise\" (up to half) or \"vor_ort\" (on site).
- regionen are places or regions where I want to work, such as Hamburg or Munich.
- branchen under einsatzpraeferenzen are industries I want to work in from now on, not simply the previous ones.
- min_tagessatz is a day rate below which I explicitly do not work.
- laender are country codes such as \"DE\", \"AT\" and \"CH\" if I limit assignments to these countries.
- ausgeschlossene_vertragsarten names \"anue\" if I rule out temporary agency work and \"festanstellung\" if I do not want a permanent role.
- verfuegbar_ab is \"sofort\" (immediately) or a date such as \"01.11.2026\", day first; a month without a day becomes the first of that month.";

    pub(super) const ANSWER: &str = "The answer
Answer only with the JSON in one single code block, without any text before or after it. Keep every key of the structure below, its spelling and the order. An object in a list shows the structure of one entry; repeat it for every entry. Write numbers without quotation marks and without a unit. The JSON must be valid, with straight double quotation marks, no comments and no comma before a closing bracket.";

    pub(super) const CHECK: &str = "Check before you answer";
    pub(super) const CHECKS: [&str; 4] = [
        "Is every value in the CV, or does it follow directly from it?",
        "Is every skill a short term, and does every term in auch mean exactly this skill?",
        "Is every focus area written exactly as in kernkompetenzen, and does every target role name a field?",
        "Do all years come from the dates in the CV?",
    ];
    pub(super) const CHECK_SETTINGS: &str =
        "Are preferences and exclusion criteria empty where nobody stated them?";
    pub(super) const CHECK_UPDATE: &str =
        "Is every entry that matches one in the current profile written exactly as there?";
    pub(super) const CHECK_ANSWER: &str =
        "Is the answer one single valid JSON code block with exactly the keys of the structure?";

    pub(super) const CURRENT: &str =
        "My current profile (JSON, without name, preferences and exclusion criteria)";
    pub(super) const STRUCTURE: &str = "The structure";

    pub(super) const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
}

/// The words of the prompt in one language.
struct Words {
    intro: &'static str,
    intro_update: &'static str,
    principles: &'static str,
    update: &'static str,
    fields: &'static str,
    settings: &'static str,
    answer: &'static str,
    check: &'static str,
    checks: [&'static str; 4],
    check_settings: &'static str,
    check_update: &'static str,
    check_answer: &'static str,
    current: &'static str,
    structure: &'static str,
    months: [&'static str; 12],
    /// Day, month name, year: `{d}`, `{m}`, `{y}` replaced.
    date: &'static str,
}

const DE: Words = Words {
    intro: INTRO,
    intro_update: INTRO_UPDATE,
    principles: PRINCIPLES,
    update: UPDATE,
    fields: FIELDS,
    settings: SETTINGS,
    answer: ANSWER,
    check: CHECK,
    checks: CHECKS,
    check_settings: CHECK_SETTINGS,
    check_update: CHECK_UPDATE,
    check_answer: CHECK_ANSWER,
    current: CURRENT,
    structure: STRUCTURE,
    months: MONTHS,
    date: "{d}. {m} {y}",
};

const EN: Words = Words {
    intro: en::INTRO,
    intro_update: en::INTRO_UPDATE,
    principles: en::PRINCIPLES,
    update: en::UPDATE,
    fields: en::FIELDS,
    settings: en::SETTINGS,
    answer: en::ANSWER,
    check: en::CHECK,
    checks: en::CHECKS,
    check_settings: en::CHECK_SETTINGS,
    check_update: en::CHECK_UPDATE,
    check_answer: en::CHECK_ANSWER,
    current: en::CURRENT,
    structure: en::STRUCTURE,
    months: en::MONTHS,
    date: "{d} {m} {y}",
};

impl Words {
    fn of(language: Language) -> &'static Words {
        match language {
            Language::De => &DE,
            Language::En => &EN,
        }
    }

    /// `25. September 2026`, `25 September 2026`.
    fn day(&self, day: Date) -> String {
        let month = usize::try_from(day.month() - 1).unwrap_or(0);
        self.date
            .replace("{d}", &day.day().to_string())
            .replace("{m}", self.months[month])
            .replace("{y}", &day.year().to_string())
    }
}

/// The whole prompt in the app's language: the task, the principles (with `today`), for an
/// update of `stored` how to treat the current profile, the rule of every field, for a new
/// profile the wishes and exclusion criteria, the answer wanted and the checks before it; then
/// the current profile (an update only) and the skeleton, each in a JSON code block.
pub fn text(language: Language, today: Date, stored: Option<&ProfileForm>) -> String {
    let w = Words::of(language);
    let mut parts = vec![
        (if stored.is_some() {
            w.intro_update
        } else {
            w.intro
        })
        .to_owned(),
        w.principles.replace("{today}", &w.day(today)),
    ];
    if stored.is_some() {
        parts.push(w.update.to_owned());
    }
    parts.push(w.fields.to_owned());
    if stored.is_none() {
        parts.push(w.settings.to_owned());
    }
    parts.push(w.answer.to_owned());
    let own = if stored.is_some() {
        w.check_update
    } else {
        w.check_settings
    };
    let checks: Vec<String> = w
        .checks
        .iter()
        .chain([&own, &w.check_answer])
        .enumerate()
        .map(|(i, check)| format!("{}. {check}", i + 1))
        .collect();
    parts.push(format!("{}\n{}", w.check, checks.join("\n")));
    if let Some(form) = stored {
        parts.push(format!("{}\n```json\n{}\n```", w.current, current(form)));
    }
    parts.push(format!(
        "{}\n```json\n{}\n```",
        w.structure,
        skeleton(stored.is_none())
    ));
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

/// The CV part of the stored profile under the skeleton's keys and in its order (no name, no
/// wishes, no criteria), laid out like the skeleton.
fn current(form: &ProfileForm) -> String {
    let objects = |key: &str, texts: &[String]| {
        Json::Array(
            texts
                .iter()
                .map(|text| {
                    let mut item = Json::object();
                    item.set(key, Json::text(text));
                    item
                })
                .collect(),
        )
    };
    let mut doc = Json::object();
    doc.set("titel", Json::text(&form.title));
    doc.set("wunschrollen", Json::texts(&form.roles));
    doc.set(
        "berufserfahrung_jahre",
        form.years.map_or(Json::Null, Json::number),
    );
    doc.set("ausbildung", objects("abschluss", &form.degrees));
    let competences = form.competences.iter().map(|row| {
        let mut item = Json::object();
        item.set("kompetenz", Json::text(&row.name));
        item.set("jahre", row.years.map_or(Json::Null, Json::number));
        item.set("auch", Json::texts(&row.aliases));
        item
    });
    doc.set("kernkompetenzen", Json::Array(competences.collect()));
    doc.set("schwerpunkte", Json::texts(&form.focus));
    doc.set("methoden_tools", objects("name", &form.tools));
    doc.set("zertifizierungen", objects("name", &form.certificates));
    doc.set("branchen", objects("branche", &form.industries));
    let languages = form.languages.iter().map(|row| {
        let mut item = Json::object();
        item.set("sprache", Json::text(&row.language));
        let level = row.level.map_or("", |level| level.text());
        item.set("niveau", Json::text(level));
        item
    });
    doc.set("sprachen", Json::Array(languages.collect()));
    doc.set("alleinstellungsmerkmale", Json::texts(&form.strengths));
    doc.set("keywords", Json::texts(&form.keywords));
    answer::drop_empty(&mut doc);
    layout(&doc)
}

/// A JSON object laid out like the skeleton: one key per line, every object of a list on a
/// line of its own.
fn layout(doc: &Json) -> String {
    let Json::Object(entries) = doc else {
        return inline(doc);
    };
    let lines: Vec<String> = entries
        .iter()
        .map(|(key, value)| {
            let key = inline(&Json::text(key));
            match value {
                Json::Array(items) if items.iter().any(Json::is_object) => {
                    let items: Vec<String> = items
                        .iter()
                        .map(|item| format!("    {}", inline(item)))
                        .collect();
                    format!("  {key}: [\n{}\n  ]", items.join(",\n"))
                }
                other => format!("  {key}: {}", inline(other)),
            }
        })
        .collect();
    format!("{{\n{}\n}}", lines.join(",\n"))
}

/// A value on one line, spaced like the skeleton (`{ "kompetenz": "Controlling" }`).
fn inline(value: &Json) -> String {
    match value {
        Json::Object(entries) if entries.is_empty() => "{}".to_owned(),
        Json::Object(entries) => {
            let fields: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{}: {}", inline(&Json::text(key)), inline(value)))
                .collect();
            format!("{{ {} }}", fields.join(", "))
        }
        Json::Array(items) => {
            let items: Vec<String> = items.iter().map(inline).collect();
            format!("[{}]", items.join(", "))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::form::{
        self, LanguageLevel, ProfileAvailability, ProfileCompetence, ProfileCriteria,
        ProfileLanguage, ProfileWishes,
    };
    use super::*;
    use crate::error::InvalidInput;
    use crate::matching::{self, ProfileWarningCode};

    /// The skeleton filled the way an AI would answer (same keys, same order).
    const FILLED: &str = r#"{
  "name": "Erika Beispiel",
  "titel": "Interim CFO",
  "wunschrollen": ["Interim CFO"],
  "berufserfahrung_jahre": 20,
  "ausbildung": [
    { "abschluss": "Diplom-Kauffrau" }
  ],
  "kernkompetenzen": [
    { "kompetenz": "Controlling", "jahre": 12, "auch": ["FP&A"] }
  ],
  "schwerpunkte": ["Controlling"],
  "methoden_tools": [
    { "name": "SAP" }
  ],
  "zertifizierungen": [
    { "name": "PMP" }
  ],
  "branchen": [
    { "branche": "Chemie" }
  ],
  "sprachen": [
    { "sprache": "Englisch", "niveau": "C1" }
  ],
  "alleinstellungsmerkmale": ["Schnell"],
  "keywords": ["IFRS"],
  "stationen": [
    { "zeitraum": "2012 - heute", "rolle": "Interim CFO", "schwerpunkte": ["Restrukturierung"] }
  ],
  "einsatzpraeferenzen": {
    "tagessatz_wunsch": 1200,
    "remote": "ueberwiegend",
    "regionen": ["Hamburg"],
    "branchen": ["Chemie"]
  },
  "harte_kriterien": {
    "min_tagessatz": 1000,
    "laender": ["DE"],
    "ausgeschlossene_vertragsarten": ["anue"],
    "verfuegbar_ab": "sofort"
  }
}"#;

    fn day() -> Date {
        Date::new(2026, 9, 25).unwrap()
    }

    /// A stored profile with every field, the settings with values found nowhere else.
    fn stored() -> ProfileForm {
        let texts = |items: &[&str]| items.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
        ProfileForm {
            name: "Erika Beispiel".into(),
            title: "Interim CFO".into(),
            competences: vec![
                ProfileCompetence {
                    name: "Konzerncontrolling".into(),
                    years: Some(12),
                    aliases: texts(&["Group Controlling"]),
                    origin: Some(0),
                },
                ProfileCompetence {
                    name: "Treasury".into(),
                    years: None,
                    aliases: Vec::new(),
                    origin: Some(1),
                },
            ],
            strengths: texts(&["Baute zwei Finanzbereiche \"aus einem Guss\" auf."]),
            keywords: texts(&["IFRS 16"]),
            years: Some(21),
            degrees: texts(&["Diplom-Kauffrau (Univ.)"]),
            industries: texts(&["Chemie"]),
            tools: texts(&["SAP S/4HANA FI"]),
            certificates: texts(&["PMP"]),
            languages: vec![
                ProfileLanguage {
                    language: "Deutsch".into(),
                    level: Some(LanguageLevel::Native),
                    origin: Some(0),
                },
                ProfileLanguage {
                    language: "Französisch".into(),
                    level: None,
                    origin: Some(1),
                },
            ],
            focus: texts(&["Konzerncontrolling"]),
            roles: texts(&["Head of Controlling"]),
            wishes: ProfileWishes {
                day_rate: Some(1234),
                remote: None,
                regions: texts(&["Kleinhausen"]),
                industries: texts(&["Raumfahrt"]),
            },
            criteria: ProfileCriteria {
                min_day_rate: Some(987),
                countries: texts(&["LU"]),
                no_anue: true,
                available: ProfileAvailability::From {
                    date: "2027-03-01".into(),
                },
                ..ProfileCriteria::default()
            },
        }
    }

    /// The keys of a document in order, the items of a list by its first one.
    fn shape(value: &Json) -> String {
        match value {
            Json::Object(entries) => entries
                .iter()
                .map(|(key, child)| format!("{key}{{{}}}", shape(child)))
                .collect::<Vec<_>>()
                .join(","),
            Json::Array(items) => items.first().map(shape).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn keys(doc: &Json) -> Vec<String> {
        match doc {
            Json::Object(entries) => entries.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Every key of a document, nested ones too, each once.
    fn all_keys(value: &Json, out: &mut Vec<String>) {
        match value {
            Json::Object(entries) => {
                for (key, child) in entries {
                    if !out.contains(key) {
                        out.push(key.clone());
                    }
                    all_keys(child, out);
                }
            }
            Json::Array(items) => items.iter().for_each(|item| all_keys(item, out)),
            _ => {}
        }
    }

    /// The rules of a prompt: everything before its JSON code blocks.
    fn rules(text: &str) -> &str {
        &text[..text.find("```json").unwrap()]
    }

    /// `word` stands in `text` as a word of its own (keys contain `_`).
    fn names(text: &str, word: &str) -> bool {
        text.match_indices(word).any(|(at, _)| {
            let before = text[..at].chars().next_back();
            let after = text[at + word.len()..].chars().next();
            let apart = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
            apart(before) && apart(after)
        })
    }

    /// Every key of the skeleton is one the editor or the engine reads: the filled skeleton
    /// fills every field of the form, its stations (kept in the file, the form does not show
    /// them) give the engine terms, and writing the form into an empty profile gives back the
    /// keys of the skeleton but the stations.
    #[test]
    fn the_skeleton_names_exactly_the_keys_of_the_form() {
        let skeleton: Json = serde_json::from_str(&skeleton(true)).unwrap();
        let doc: Json = serde_json::from_str(FILLED).unwrap();
        assert_eq!(
            shape(&doc),
            shape(&skeleton),
            "the answer keeps the skeleton"
        );
        let read = form::read(&doc);
        let c = &read.criteria;
        let w = &read.wishes;
        for (field, is_empty) in [
            ("name", read.name.is_empty()),
            ("title", read.title.is_empty()),
            ("roles", read.roles.is_empty()),
            ("years", read.years.is_none()),
            ("degrees", read.degrees.is_empty()),
            ("competences", read.competences.is_empty()),
            ("aliases", read.competences[0].aliases.is_empty()),
            ("tools", read.tools.is_empty()),
            ("certificates", read.certificates.is_empty()),
            ("industries", read.industries.is_empty()),
            ("languages", read.languages[0].level.is_none()),
            ("strengths", read.strengths.is_empty()),
            ("keywords", read.keywords.is_empty()),
            ("focus", read.focus.is_empty()),
            ("wishDayRate", w.day_rate.is_none()),
            ("remote", w.remote.is_none()),
            ("regions", w.regions.is_empty()),
            ("wishIndustries", w.industries.is_empty()),
            ("minDayRate", c.min_day_rate.is_none()),
            ("countries", c.countries.is_empty()),
            ("noAnue", !c.no_anue),
            ("available", c.available == form::ProfileAvailability::Unset),
        ] {
            assert!(!is_empty, "{field} not read from the skeleton");
        }
        let summary = matching::compile_profile(&doc.to_value()).summary().clone();
        assert!(
            summary
                .sources
                .iter()
                .any(|s| s.path.starts_with("stationen[]")),
            "{:?}",
            summary.sources
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);

        // Two degrees go into `ausbildung`, as the skeleton has them.
        let mut form = read.clone();
        form.degrees.push("MBA".into());
        let mut written = Json::object();
        form::merge(&mut written, &form::ProfileForm::default(), &form, &[]);
        let mut expected = keys(&skeleton);
        expected.retain(|key| key != "stationen");
        assert_eq!(keys(&written), expected);

        // The skeleton of an update is the same without the settings.
        let update: Json = serde_json::from_str(&super::skeleton(false)).unwrap();
        expected.retain(|key| key != "einsatzpraeferenzen" && key != "harte_kriterien");
        expected.push("stationen".into());
        assert_eq!(keys(&update), expected);
    }

    /// What the AI leaves as in the skeleton is dropped: the unfilled skeleton holds no
    /// profile, and a partly filled one leaves no empty criterion the app cannot read.
    #[test]
    fn an_answer_without_values_leaves_no_empty_keys() {
        for settings in [true, false] {
            assert_eq!(
                super::super::draft_from_answer(&skeleton(settings)).map(|d| d.form),
                Err(InvalidInput::ProfileAnswer)
            );
        }
        let answer = skeleton(true).replacen("\"name\": \"\"", "\"name\": \"Erika Beispiel\"", 1);
        let draft = super::super::draft_from_answer(&answer).unwrap();
        assert_eq!(draft.source, "{\n  \"name\": \"Erika Beispiel\"\n}");
        assert!(
            !draft
                .summary
                .warnings
                .iter()
                .any(|w| w.code == ProfileWarningCode::CriterionNotUnderstood),
            "{:?}",
            draft.summary.warnings
        );
        // The filled skeleton comes back as it is.
        let draft = super::super::draft_from_answer(&format!("```json\n{FILLED}\n```")).unwrap();
        assert_eq!(draft.source, FILLED);
    }

    /// Every key of the skeleton has its rule before the code blocks, in both languages; an
    /// update asks for nothing of the user's settings.
    #[test]
    fn every_key_of_the_skeleton_is_explained() {
        for language in [Language::De, Language::En] {
            for update in [false, true] {
                let stored = stored();
                let text = text(language, day(), update.then_some(&stored));
                let skeleton: Json = serde_json::from_str(&skeleton(!update)).unwrap();
                let mut keys = Vec::new();
                all_keys(&skeleton, &mut keys);
                for key in &keys {
                    assert!(names(rules(&text), key), "{language:?} {update}: {key}");
                }
                if update {
                    let full: Json = serde_json::from_str(&super::skeleton(true)).unwrap();
                    let mut settings = Vec::new();
                    all_keys(&full, &mut settings);
                    settings.retain(|key| !keys.contains(key));
                    assert_eq!(settings.len(), 9, "{settings:?}");
                    for key in settings {
                        assert!(!names(&text, &key), "{language:?}: {key}");
                    }
                }
                // The fixed values the form reads.
                let values: &[&str] = if update {
                    &["Muttersprache", "C1", "B2", "A2"]
                } else {
                    &[
                        "Muttersprache",
                        "voll",
                        "ueberwiegend",
                        "teilweise",
                        "vor_ort",
                        "anue",
                        "festanstellung",
                        "sofort",
                    ]
                };
                for value in values {
                    assert!(
                        text.contains(&format!("\"{value}\"")),
                        "{language:?}: {value}"
                    );
                }
            }
        }
    }

    /// German and English say the same thing in the same order: the same lines, each rule
    /// on the same key, the same numbered steps, the same JSON.
    #[test]
    fn both_languages_have_the_same_structure() {
        let stored = stored();
        for update in [None, Some(&stored)] {
            let german = text(Language::De, day(), update);
            let english = text(Language::En, day(), update);
            let (de, en): (Vec<&str>, Vec<&str>) =
                (german.lines().collect(), english.lines().collect());
            assert_eq!(de.len(), en.len(), "{german}\n{english}");
            let mut code = false;
            for (d, e) in de.iter().zip(&en) {
                if d.starts_with("```") {
                    code = !code;
                    assert_eq!(d, e);
                } else if code {
                    assert_eq!(d, e, "the JSON is the same");
                } else if let Some(rule) = d.strip_prefix("- ") {
                    let key = |line: &str| line.split(' ').next().unwrap().to_owned();
                    assert_eq!(Some(key(rule)), e.strip_prefix("- ").map(key), "{d}\n{e}");
                } else if let Some((number, _)) =
                    d.split_once(". ").filter(|(n, _)| n.parse::<u8>().is_ok())
                {
                    assert!(e.starts_with(&format!("{number}. ")), "{d}\n{e}");
                } else {
                    assert_eq!(d.is_empty(), e.is_empty(), "{d}\n{e}");
                }
            }
            assert!(!code, "every code block closes");
        }
    }

    /// The prompt says which day it is, ends with the skeleton in a code block and asks for
    /// one code block back; it reads like the app's other prompts.
    #[test]
    fn the_text_says_today_and_ends_with_the_skeleton() {
        let german = text(Language::De, day(), None);
        assert!(german.starts_with("Du unterstützt mich als KI-Assistent"));
        assert!(german.contains("Heute ist der 25. September 2026."));
        assert!(german.contains("in einem einzigen Codeblock"));
        assert!(german.ends_with(&format!("Der Aufbau\n```json\n{}\n```\n", skeleton(true))));
        let english = text(Language::En, day(), None);
        assert!(english.starts_with("You support me as an AI assistant"));
        assert!(english.contains("Today is 25 September 2026."));
        assert!(english.contains("in one single code block"));
        assert!(english.ends_with(&format!(
            "The structure\n```json\n{}\n```\n",
            skeleton(true)
        )));
        let march = Date::new(2027, 3, 1).unwrap();
        assert!(text(Language::De, march, None).contains("Heute ist der 1. März 2027."));

        for text in [&german, &english] {
            // No dash as a separator, no product named.
            for word in [
                "\u{2013}", "\u{2014}", "Claude", "ChatGPT", "Gemini", "Copilot",
            ] {
                assert!(!text.contains(word), "{word}");
            }
        }
        // The English one speaks English: German only in keys, fixed values and the examples
        // of German degrees and terms.
        for word in [
            "und",
            "der",
            "die",
            "das",
            "nicht",
            "ist",
            "mit",
            "von",
            "für",
            "oder",
            "wenn",
            "Lebenslauf",
        ] {
            assert!(!names(rules(&english), word), "{word}");
        }
        for word in ["the", "and", "you", "your", "with"] {
            assert!(!names(rules(&german), word), "{word}");
        }
    }

    /// An update shows the CV part of the stored profile (as JSON the editor reads back the
    /// same) and nothing of the person or the user's settings.
    #[test]
    fn the_update_carries_the_cv_part_of_the_stored_profile_only() {
        let stored = stored();
        for language in [Language::De, Language::En] {
            let text = text(language, day(), Some(&stored));
            let heading = if language == Language::De {
                "Mein bisheriges Profil"
            } else {
                "My current profile"
            };
            let block = &text[text.find(heading).unwrap()..];
            let block = &block[block.find("```json\n").unwrap() + 8..];
            let block = &block[..block.find("\n```").unwrap()];
            let doc: Json = serde_json::from_str(block).unwrap_or_else(|e| panic!("{e}: {block}"));
            let expected = ProfileForm {
                name: String::new(),
                wishes: ProfileWishes::default(),
                criteria: ProfileCriteria::default(),
                ..stored.clone()
            };
            assert_eq!(form::read(&doc), expected);
            // Laid out like the skeleton: one entry of a list per line.
            assert!(
                block.contains(
                    "    { \"kompetenz\": \"Konzerncontrolling\", \"jahre\": 12, \"auch\": [\"Group Controlling\"] },\n    { \"kompetenz\": \"Treasury\" }\n"
                ),
                "{block}"
            );
            assert!(
                block.contains("{ \"sprache\": \"Französisch\" }"),
                "{block}"
            );
            assert!(block.contains("\\\"aus einem Guss\\\""), "{block}");
            for private in [
                "Erika",
                "1234",
                "987",
                "Kleinhausen",
                "Raumfahrt",
                "LU",
                "2027",
            ] {
                assert!(!names(&text, private), "{language:?}: {private}");
            }
        }
    }
}
