//! Word lists of the new engine.
//!
//! external contract - do not translate: German and English wording of job ads and
//! consultant profiles. All entries are case-folded; sorted tables are searched with
//! `binary_search`, keep them sorted.

/// Words that carry no skill on top of the old stopwords (sorted).
pub(crate) const FILLERS: &[&str] = &[
    "ability",
    "abstimmung",
    "aktuelle",
    "analytische",
    "anspruchsvolle",
    "anwendung",
    "aufgaben",
    "ausgepragt",
    "background",
    "bereich",
    "bereichen",
    "bereits",
    "besten",
    "comparable",
    "deep",
    "detaillierte",
    "eigene",
    "einem",
    "entsprechende",
    "erfahren",
    "erfahrene",
    "erfahrungen",
    "experienced",
    "expertise",
    "familiarity",
    "gangigen",
    "general",
    "ggf",
    "hands-on",
    "hohe",
    "hoher",
    "idealerweise",
    "inkl",
    "insbesondere",
    "jahre",
    "jahren",
    "kenntnissen",
    "least",
    "level",
    "mind",
    "mindestens",
    "moderne",
    "more",
    "nachweisbare",
    "nachweislich",
    "niveau",
    "record",
    "several",
    "sicher",
    "sound",
    "starke",
    "strong",
    "tatigkeit",
    "track",
    "umfeld",
    "umgang",
    "understanding",
    "unternehmensumfeld",
    "usw",
    "vergleichbare",
    "vergleichbaren",
    "vertiefte",
    "very",
    "vorzugsweise",
    "weitreichende",
    "within",
    "worked",
    "working",
    "year",
    "zudem",
];

/// Atoms that never meet a requirement on their own (sorted).
pub(crate) const GENERIC_ATOMS: &[&str] = &[
    "analyse",
    "berater",
    "beratung",
    "business",
    "consultant",
    "consulting",
    "digital",
    "finance",
    "finanz",
    "it",
    "leitung",
    "management",
    "manager",
    "projekt",
    "prozess",
    "sap",
    "system",
    "team",
    "technologie",
    "tool",
    "werkzeug",
];

/// Compound modifiers that do not narrow a skill (`Konzernkonsolidierung` is still
/// consolidation) (sorted).
pub(crate) const LIGHT_MODIFIERS: &[&str] = &[
    "bereich",
    "business",
    "finanz",
    "gesamt",
    "gruppen",
    "konzern",
    "projekt",
    "sap",
    "unternehmen",
    "unternehmens",
];

/// Compound heads that do not change a skill (`Carve-out-Projekten`,
/// `Restrukturierungsumfeld`) (sorted).
pub(crate) const LIGHT_HEADS: &[&str] = &[
    "aufgabe",
    "bereich",
    "einsatz",
    "erfahrung",
    "kenntnis",
    "projekt",
    "prozess",
    "thema",
    "umfeld",
    "vorhaben",
];

/// Bilingual synonyms: phrase (space-separated stems) -> concept stem. Longer phrases first.
pub(crate) const CONCEPTS: &[(&str, &str)] = &[
    ("annual financial statement", "jahresabschluss"),
    ("month-end closing", "monatsabschluss"),
    ("month end closing", "monatsabschluss"),
    ("monthly closing", "monatsabschluss"),
    ("year-end closing", "jahresabschluss"),
    ("group accounting", "konzernrechnungslegung"),
    ("konzernrechnungswesen", "konzernrechnungslegung"),
    ("group reporting", "konzernreporting"),
    ("project management", "projektmanagement"),
    ("project manag", "projektmanagement"),
    ("programme management", "programmmanagement"),
    ("program management", "programmmanagement"),
    ("change management", "changemanagement"),
    ("change managment", "changemanagement"),
    ("stakeholder management", "stakeholdermanagement"),
    ("liquidity planning", "liquiditatsplanung"),
    ("data migration", "datenmigration"),
    ("test management", "testmanagement"),
    ("master data", "stammdat"),
    ("post merger integration", "post-merger-integration"),
    ("shared service centre", "shared-service-cent"),
    ("shared service center", "shared-service-cent"),
    ("order to cash", "order-to-cash"),
    ("procure to pay", "procure-to-pay"),
    ("project lead", "projektleitung"),
    ("interim management", "interimmanagement"),
    ("working capital management", "working-capital-management"),
    ("working capital", "working-capital-management"),
    ("consolidation", "konsolidierung"),
    ("controller", "controlling"),
    ("budgeting", "budgetierung"),
    ("budgetplanung", "budgetierung"),
    ("forecasting", "forecast"),
    ("restructuring", "restrukturierung"),
    ("projektleiter", "projektleitung"),
    ("projektleiterin", "projektleitung"),
    ("gesamtprojektleitung", "projektleitung"),
    ("programmleitung", "programmmanagement"),
    ("veranderungsmanagement", "changemanagement"),
    ("implementation", "einfuhrung"),
    ("implementierung", "einfuhrung"),
    ("migration", "migration"),
    ("german", "deutsch"),
    ("english", "englisch"),
    ("french", "franzosisch"),
    ("spanish", "spanisch"),
    ("otc", "order-to-cash"),
    ("pmi", "post-merger-integration"),
    ("s4hana", "s/4hana"),
    ("s4", "s/4hana"),
];

/// Language stems and their canonical name.
pub(crate) const LANGUAGES: &[&str] = &[
    "deutsch",
    "englisch",
    "franzosisch",
    "spanisch",
    "italienisch",
    "niederlandisch",
    "polnisch",
    "russisch",
    "chinesisch",
    "portugiesisch",
    "turkisch",
];

/// Language level words (case-folded, without umlauts) and CEFR level 1 (A1) .. 7 (native).
pub(crate) const LEVEL_WORDS: &[(&str, u8)] = &[
    ("a1", 1),
    ("a2", 2),
    ("b1", 3),
    ("b2", 4),
    ("c1", 5),
    ("c2", 6),
    ("grundkenntnisse", 2),
    ("basic", 2),
    ("gute", 4),
    ("gut", 4),
    ("good", 4),
    ("sehr gute", 5),
    ("verhandlungssicher", 5),
    ("verhandlungssichere", 5),
    ("fliessend", 5),
    ("fliessende", 5),
    ("fluent", 5),
    ("business fluent", 5),
    ("proficient", 5),
    ("muttersprache", 7),
    ("muttersprachlich", 7),
    ("muttersprachliche", 7),
    ("native", 7),
    ("mother tongue", 7),
];

/// Soft skills: weight 250, cannot be proven from a profile (prefix stems, sorted).
pub(crate) const SOFT_SKILLS: &[&str] = &[
    "analytisch",
    "belastbar",
    "communication",
    "durchsetzung",
    "eigeninitiative",
    "empathie",
    "engagement",
    "flexibilit",
    "hands-on-mental",
    "kommunikation",
    "kundenorientier",
    "losungsorientier",
    "motivation",
    "organisationstalent",
    "proaktiv",
    "selbststandig",
    "sozialkompetenz",
    "teamfahig",
    "teamplayer",
    "verhandlungsgeschick",
    "zuverlassig",
];

/// Frame conditions: weight 0 (travel, availability, presence, driving licence) (prefix
/// stems, sorted).
pub(crate) const FRAME_WORDS: &[&str] = &[
    "availability",
    "bereitschaft",
    "fuhrerschein",
    "reise",
    "reisebereitschaft",
    "start",
    "travel",
    "verfugbar",
    "vor-ort-prasenz",
    "willingness",
];

/// Formal requirements: degrees (prefix stems; a bare `Abschluss` is also a financial
/// statement, so it does not count).
pub(crate) const DEGREE_WORDS: &[&str] = &[
    "bachelor",
    "degree",
    "diplom",
    "hochschulabschluss",
    "master",
    "studium",
    "university",
];

/// Degree fields: stems in a requirement or profile degree -> field id.
pub(crate) const DEGREE_FIELDS: &[(&str, &str)] = &[
    ("betriebswirt", "business"),
    ("bwl", "business"),
    ("business", "business"),
    ("kauffrau", "business"),
    ("kaufmann", "business"),
    ("okonom", "business"),
    ("wirtschaftsinformat", "business-it"),
    ("wirtschaftswissenschaft", "business"),
    ("finance", "business"),
    ("informatik", "it"),
    ("computer science", "it"),
    ("ingenieur", "engineering"),
];

/// A degree requirement that accepts any comparable degree.
pub(crate) const COMPARABLE_WORDS: &[&str] = &["vergleichbar", "comparable", "equivalent"];
/// General experience, compared with the profile's total years.
pub(crate) const GENERAL_EXPERIENCE: &[&str] =
    &["berufserfahrung", "berufspraxis", "praxis", "professional"];
/// Year units after a number (`10 Jahre`, `8 years`).
pub(crate) const YEAR_UNITS: &[&str] = &["jahr", "year"];
/// Numbers written as words.
pub(crate) const NUMBER_WORDS: &[(&str, u32)] = &[
    ("zwei", 2),
    ("drei", 3),
    ("vier", 4),
    ("funf", 5),
    ("sechs", 6),
    ("sieben", 7),
    ("acht", 8),
    ("neun", 9),
    ("zehn", 10),
    ("zwolf", 12),
    ("funfzehn", 15),
    ("zwanzig", 20),
    ("two", 2),
    ("three", 3),
    ("five", 5),
    ("ten", 10),
];

/// Profile keys of the new engine (German JSON keys shared with the skill).
pub(crate) const KEY_LANGUAGES: &str = "sprachen";
pub(crate) const KEY_LANGUAGE: &str = "sprache";
pub(crate) const KEY_LEVEL: &str = "niveau";
pub(crate) const KEYS_YEARS: &[&str] = &["jahre", "years", "erfahrung_jahre"];

/// Extra must headings (normalised heading prefixes).
pub(crate) const MUST_PREFIXES: &[&str] = &[
    "ihre qualifikation",
    "deine qualifikation",
    "your profile",
    "what you bring",
    "requirements",
    "your skills",
    "must-have",
    "must have",
    "anforderungsprofil",
    "profil",
];
/// Extra nice headings.
pub(crate) const NICE_PREFIXES: &[&str] = &[
    "preferred",
    "nice to have",
    "nice-to-have",
    "good to have",
    "a plus",
];
/// Headings that end requirement sections.
pub(crate) const OTHER_PREFIXES: &[&str] = &[
    "rahmenbedingungen",
    "konditionen",
    "eckdaten",
    "wir bieten",
    "what we offer",
    "about",
    "benefits",
    "zum ablauf",
    "bewerbung",
    "your responsibilities",
    "responsibilities",
    "ihre aufgaben",
    "deine aufgaben",
    "projektbeschreibung",
    "ausgangslage",
    "das programmteam",
];
/// A must line containing one of these is a nice-to-have.
pub(crate) const NICE_CUES: &[&str] = &[
    "von vorteil",
    "wunschenswert",
    "idealerweise",
    "nice to have",
    "nice-to-have",
    "ideally",
    "a plus",
    "vorteilhaft",
    "bonus",
];
/// English requirement cues for the sentence stage.
pub(crate) const EN_CUES: &[&str] = &[
    "required",
    "experience in",
    "experience with",
    "knowledge of",
    "you bring",
];
/// Example markers inside an item (alternatives of the head).
pub(crate) const EXAMPLES: &[&str] = &[
    "z. b.",
    "z.b.",
    "u. a.",
    "u.a.",
    "e.g.",
    "e. g.",
    "such as",
    "etwa",
    "beispielsweise",
];
/// Separators inside an example list.
pub(crate) const EXAMPLE_SEPARATORS: &[&str] = &[", ", " und ", " oder ", " or ", " and "];
/// Abbreviations that end with a dot but not a sentence.
pub(crate) const ABBREVIATIONS: &[&str] = &[
    "bzw", "ca", "dr", "evtl", "ggf", "inkl", "max", "min", "nr", "vgl", "zzgl",
];
/// AND separators of requirement items.
pub(crate) const AND: &[&str] = &[
    ", ",
    "; ",
    " und ",
    " sowie ",
    " & ",
    " and ",
    " as well as ",
    " inkl. ",
    " including ",
];
/// Words that make a phrase an AND list.
pub(crate) const AND_WORDS: &[&str] = &[" und ", " sowie ", " and ", " & "];
/// OR separators: alternatives inside an item.
pub(crate) const OR: &[&str] = &[" oder ", " bzw. ", " beziehungsweise ", " or ", " / "];

/// Rate statements, units and currencies.
pub(crate) const RATE_WORDS: &[&str] = &[
    "tagessatz",
    "honorar",
    "stundensatz",
    "day rate",
    "daily rate",
    "hourly rate",
    "vergutung",
    "pro tag",
    "per day",
    "pro stunde",
    "per hour",
    "€/tag",
    "€/h",
];
pub(crate) const SALARY_WORDS: &[&str] = &["gehalt", "salary"];
pub(crate) const HOURLY_WORDS: &[&str] = &["stunde", "std", "hour", "/h", "stundensatz"];
pub(crate) const OTHER_CURRENCIES: &[&str] = &["chf", "usd", "gbp", "$", "£"];
/// Sentences that state a start.
pub(crate) const START_WORDS: &[&str] = &[
    "start",
    "beginn",
    "projektstart",
    "verfugbarkeit",
    "ab sofort",
    "asap",
    "starting",
];
/// "Remote from <country>".
pub(crate) const REMOTE_FROM: &[&str] = &["remote aus", "remote from"];
/// The consultant is available now.
pub(crate) const NOW_WORD: &str = "sofort";

/// Month names -> month number (case-folded, without umlauts).
pub(crate) const MONTHS: &[(&str, i8)] = &[
    ("januar", 1),
    ("january", 1),
    ("februar", 2),
    ("february", 2),
    ("marz", 3),
    ("march", 3),
    ("april", 4),
    ("mai", 5),
    ("may", 5),
    ("juni", 6),
    ("june", 6),
    ("juli", 7),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("oktober", 10),
    ("october", 10),
    ("november", 11),
    ("dezember", 12),
    ("december", 12),
];

/// Words meaning "start now".
pub(crate) const START_NOW: &[&str] = &["sofort", "asap", "immediately", "ab sofort"];
/// Words meaning "start to be agreed".
pub(crate) const START_VAGUE: &[&str] = &[
    "nach abstimmung",
    "nach absprache",
    "nach vereinbarung",
    "flexibel",
    "tbd",
    "to be agreed",
    "zeitnah",
];

/// Country names and their ISO codes (case-folded, without umlauts).
pub(crate) const COUNTRIES: &[(&str, &str)] = &[
    ("austria", "AT"),
    ("belgien", "BE"),
    ("czech", "CZ"),
    ("deutschland", "DE"),
    ("england", "GB"),
    ("frankreich", "FR"),
    ("france", "FR"),
    ("germany", "DE"),
    ("india", "IN"),
    ("indien", "IN"),
    ("italien", "IT"),
    ("italy", "IT"),
    ("luxemburg", "LU"),
    ("netherlands", "NL"),
    ("niederlande", "NL"),
    ("oesterreich", "AT"),
    ("osterreich", "AT"),
    ("poland", "PL"),
    ("polen", "PL"),
    ("schweiz", "CH"),
    ("spain", "ES"),
    ("spanien", "ES"),
    ("switzerland", "CH"),
    ("tschechien", "CZ"),
    ("uk", "GB"),
    ("usa", "US"),
];

/// Cities outside Germany that job locations name without a country (sorted).
pub(crate) const CITIES: &[(&str, &str)] = &[
    ("basel", "CH"),
    ("bern", "CH"),
    ("brussel", "BE"),
    ("graz", "AT"),
    ("innsbruck", "AT"),
    ("krakau", "PL"),
    ("linz", "AT"),
    ("london", "GB"),
    ("luxembourg", "LU"),
    ("paris", "FR"),
    ("prag", "CZ"),
    ("salzburg", "AT"),
    ("warschau", "PL"),
    ("warsaw", "PL"),
    ("wien", "AT"),
    ("zug", "CH"),
    ("zurich", "CH"),
];

/// Words that turn a country mention into a work location.
pub(crate) const ONSITE_WORDS: &[&str] = &[
    "einsatz vor ort",
    "einsatzort",
    "vor ort",
    "on site",
    "on-site",
    "onsite",
];
/// Words that make a country mention travel (a check, never decided).
pub(crate) const TRAVEL_WORDS: &[&str] = &["reise", "reisen", "travel", "workshops an"];
/// Words for "fully remote".
pub(crate) const FULL_REMOTE: &[&str] = &[
    "100 % remote",
    "100% remote",
    "fully remote",
    "vollstandig remote",
    "voll remote",
    "remote only",
];

/// ANÜ named: whole words, then substrings (case-folded, without umlauts).
pub(crate) const ANUE_WORDS: &[&str] = &["anu", "anue"];
pub(crate) const ANUE_PARTS: &[&str] = &["uberlassung", "temporary agency"];
/// ANÜ in substance without the name (whole words).
pub(crate) const ANUE_HIDDEN: &[&str] = &["payrolling", "equal pay", "igz", "bap", "gvp"];
/// Negations in the same sentence: whole words, then substrings.
pub(crate) const ANUE_NEGATION: &[&str] = &["kein", "keine", "nicht", "ohne", "not", "no"];
pub(crate) const ANUE_NEGATION_PARTS: &[&str] = &["ausgeschlossen"];
/// ANÜ only one option: whole words, then substrings.
pub(crate) const ANUE_OPTION: &[&str] = &["oder", "or", "wahlweise", "alternativ", "optional"];
pub(crate) const ANUE_OPTION_PARTS: &[&str] = &["je nach", "moglich"];

/// Permanent position.
pub(crate) const PERMANENT_WORDS: &[&str] = &[
    "festanstellung",
    "jahresgehalt",
    "unbefristet",
    "permanent position",
    "permanent role",
    "annual salary",
];
