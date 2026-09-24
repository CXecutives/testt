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
    "proven",
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

/// Atoms that never meet a requirement on their own (sorted; stemmed forms such as
/// `financ`, `manag` included, since atoms are stemmed).
pub(crate) const GENERIC_ATOMS: &[&str] = &[
    "analys",
    "analyse",
    "berat",
    "berater",
    "beratung",
    "business",
    "consult",
    "consultant",
    "consulting",
    "digital",
    "einfuhrung",
    "erp",
    "financ",
    "finance",
    "finanz",
    "it",
    "leitung",
    "manag",
    "management",
    "manager",
    "process",
    "project",
    "projekt",
    "prozess",
    "sap",
    "system",
    "team",
    "technologi",
    "technologie",
    "tool",
    "unterstutzung",
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

/// Core bilingual synonyms and paraphrases of general business work (every profile):
/// phrase (space-separated words) -> concept. Field-specific pairs live in `domains`.
pub(crate) const CORE_CONCEPTS: &[(&str, &str)] = &[
    ("project management", "projektmanagement"),
    ("project manag", "projektmanagement"),
    ("programme management", "programmmanagement"),
    ("program management", "programmmanagement"),
    ("change management", "changemanagement"),
    ("change managment", "changemanagement"),
    ("stakeholder management", "stakeholdermanagement"),
    ("project lead", "projektleitung"),
    ("interim management", "interimmanagement"),
    ("interim manag", "interimmanagement"),
    ("interim mandat", "interimmanagement"),
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
    "auslastung",
    "availability",
    "bereitschaft",
    "dauer",
    "duration",
    "einsatzort",
    "freelancer",
    "fuhrerschein",
    "gehalt",
    "honorar",
    "hybrid",
    "laufzeit",
    "location",
    "monday",
    "on-site",
    "onsite",
    "permanent",
    "prasenz",
    "rate",
    "reise",
    "reisebereitschaft",
    "remote",
    "salary",
    "standort",
    "start",
    "stundensatz",
    "tagessatz",
    "travel",
    "verfugbar",
    "vergutung",
    "vertragsart",
    "vor-ort-prasenz",
    "willingness",
    "workload",
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

/// Degree fields: stems in a requirement or profile degree -> field id. A match inside a
/// longer match (`informatik` in `wirtschaftsinformatik`) does not count.
pub(crate) const DEGREE_FIELDS: &[(&str, &str)] = &[
    ("betriebswirt", "business"),
    ("bwl", "business"),
    ("business administration", "business"),
    ("business", "business"),
    ("kauffrau", "business"),
    ("kaufmann", "business"),
    ("okonom", "business"),
    ("economics", "business"),
    ("volkswirt", "business"),
    ("accounting", "business"),
    ("wirtschaftsinformat", "business-it"),
    ("business informatics", "business-it"),
    ("information systems", "business-it"),
    ("wirtschaftswissenschaft", "business"),
    ("finance", "business"),
    ("informatik", "it"),
    ("computer science", "it"),
    ("wirtschaftsingenieur", "business-engineering"),
    ("ingenieur", "engineering"),
    ("engineering", "engineering"),
    ("maschinenbau", "engineering"),
    ("elektrotechnik", "engineering"),
    ("verfahrenstechnik", "engineering"),
    ("technisches studium", "engineering"),
    ("rechtswissenschaft", "law"),
    ("jurist", "law"),
    ("jura", "law"),
    ("mathematik", "science"),
    ("physik", "science"),
    ("naturwissenschaft", "science"),
];

/// Neighbouring degree fields: a degree in one half-meets a requirement for the other.
pub(crate) const DEGREE_RELATED: &[(&str, &str)] = &[
    ("business", "business-it"),
    ("it", "business-it"),
    ("business", "business-engineering"),
    ("engineering", "business-engineering"),
];

/// Degree levels: 1 bachelor, 2 master or university diploma, 3 doctorate.
pub(crate) const DEGREE_LEVELS: &[(&str, u8)] = &[
    ("bachelor", 1),
    ("b.sc", 1),
    ("b. sc", 1),
    ("master", 2),
    ("m.sc", 2),
    ("m. sc", 2),
    ("mba", 2),
    ("diplom", 2),
    ("magister", 2),
    ("staatsexamen", 2),
    ("promotion", 3),
    ("phd", 3),
];
/// A university of applied sciences or a dual study: a diploma is a bachelor level.
pub(crate) const DEGREE_APPLIED: &[&str] = &[
    "(fh)",
    "(ba)",
    "fachhochschul",
    "duale hochschule",
    "berufsakademie",
];

/// Wording that makes a formal requirement mandatory.
pub(crate) const MANDATORY_WORDS: &[&str] = &[
    "zwingend",
    "unabdingbar",
    "unerlasslich",
    "unbedingt erforderlich",
    "verpflichtend",
    "mandatory",
    "is a must",
    "must-have",
];
/// Licences and admissions that cannot be acquired within a project.
pub(crate) const LICENCE_WORDS: &[&str] = &[
    "steuerberater",
    "wirtschaftsprufer",
    "rechtsanwalt",
    "volljurist",
    "approbation",
    "certified public accountant",
    "chartered accountant",
];
/// A licence word counts only as a qualification (`Zulassung als ...`, `... examen`).
pub(crate) const LICENCE_CONTEXT: &[&str] = &[
    "zulassung",
    "bestellung",
    "examen",
    "titel",
    "qualifikation",
    "als ",
    "license",
    "licence",
    "qualified",
    "volljurist",
    "approbation",
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
/// Free-text USPs (`alleinstellungsmerkmale`).
pub(crate) const KEY_USP: &str = "alleinstellungsmerkmal";

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
    "why join",
    "why us",
    "your benefits",
    "unser angebot",
    "das bieten wir",
    "ihre aufgaben",
    "deine aufgaben",
    "projektbeschreibung",
    "ausgangslage",
    "das programmteam",
    // Frame blocks and portal footers (they are no requirements).
    "engagement details",
    "compensation",
    "conditions",
    "project details",
    "projektdaten",
    "projektdetails",
    "key facts",
    "the role",
    "your mission",
    "what you will do",
    "rahmendaten",
    "vertragsdetails",
    "contract details",
    "details",
    "eckpunkte",
    "sonstiges",
    "seniority level",
    "employment type",
    "job function",
    "industries",
    "bitte beachten",
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
    ("france", "FR"),
    ("frankreich", "FR"),
    ("germany", "DE"),
    ("grossbritannien", "GB"),
    ("hungary", "HU"),
    ("india", "IN"),
    ("indien", "IN"),
    ("ireland", "IE"),
    ("irland", "IE"),
    ("italien", "IT"),
    ("italy", "IT"),
    ("luxemburg", "LU"),
    ("netherlands", "NL"),
    ("niederlande", "NL"),
    ("oesterreich", "AT"),
    ("osterreich", "AT"),
    ("poland", "PL"),
    ("polen", "PL"),
    ("portugal", "PT"),
    ("romania", "RO"),
    ("rumanien", "RO"),
    ("schweden", "SE"),
    ("schweiz", "CH"),
    ("spain", "ES"),
    ("spanien", "ES"),
    ("sweden", "SE"),
    ("switzerland", "CH"),
    ("tschechien", "CZ"),
    ("uk", "GB"),
    ("ungarn", "HU"),
    ("united kingdom", "GB"),
    ("united states", "US"),
    ("usa", "US"),
    ("vereinigte staaten", "US"),
    ("vereinigtes konigreich", "GB"),
];

/// Cities outside Germany that job locations name without a country (sorted).
pub(crate) const CITIES: &[(&str, &str)] = &[
    ("amsterdam", "NL"),
    ("bangalore", "IN"),
    ("basel", "CH"),
    ("bern", "CH"),
    ("brussel", "BE"),
    ("bucharest", "RO"),
    ("budapest", "HU"),
    ("bukarest", "RO"),
    ("chicago", "US"),
    ("cluj-napoca", "RO"),
    ("dublin", "IE"),
    ("graz", "AT"),
    ("innsbruck", "AT"),
    ("krakau", "PL"),
    ("linz", "AT"),
    ("lisbon", "PT"),
    ("lissabon", "PT"),
    ("london", "GB"),
    ("luxembourg", "LU"),
    ("madrid", "ES"),
    ("new york", "US"),
    ("paris", "FR"),
    ("prag", "CZ"),
    ("pune", "IN"),
    ("salzburg", "AT"),
    ("warsaw", "PL"),
    ("warschau", "PL"),
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
pub(crate) const ANUE_NEGATION_PARTS: &[&str] = &["ausgeschlossen", "abgrenzung"];
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

/// Interim or freelance work (substrings of the folded text).
pub(crate) const INTERIM_CUES: &[&str] = &[
    "interim",
    "freiberuf",
    "freelance",
    "werkvertrag",
    "dienstvertrag",
    "projektanfrage",
    "projektlaufzeit",
    "projektdauer",
    "einsatzdauer",
    "dauer:",
    "duration",
    "auslastung",
    "tagessatz",
    "stundensatz",
    "honorar",
    "day rate",
    "daily rate",
    "hourly rate",
    "(contract)",
    "contract role",
    "contract basis",
    "contractor",
    "auf zeit",
    "projektbasis",
    "project basis",
];
/// A stated permanent position (in addition to [`PERMANENT_WORDS`]).
pub(crate) const PERMANENT_STATED: &[&str] = &[
    "festangestellt",
    "permanent contract",
    "permanent employment",
    "permanent full-time",
    "full-time permanent",
    "annual gross salary",
    "gross annual salary",
    "zielgehalt",
    "bruttojahresgehalt",
];
/// A permanent position denied (`this is not a permanent position`).
pub(crate) const PERMANENT_NEGATED: &[&str] = &[
    "not a permanent",
    "no permanent",
    "keine festanstellung",
    "nicht um eine festanstellung",
    "keine feste anstellung",
];
/// A permanent position only as a later option (`Übernahme in eine Festanstellung denkbar`).
pub(crate) const PERMANENT_OPTION: &[&str] = &[
    "ubernahme in eine festanstellung",
    "ubernahme in festanstellung",
    "option auf festanstellung",
    "option auf eine festanstellung",
    "spatere festanstellung",
    "anschliessende festanstellung",
    "possibility of a permanent",
    "option of a permanent",
    "temp-to-perm",
    "temp to perm",
];
/// A contract type line (`Vertragsart: Festanstellung`).
pub(crate) const CONTRACT_LINES: &[&str] = &["vertragsart:", "anstellungsart:", "employment type:"];
/// Indirect hints of a permanent position (benefits, work permit, career page).
pub(crate) const PERMANENT_HINTS: &[&str] = &[
    "why join",
    "work permit",
    "arbeitserlaubnis",
    "aufenthaltstitel",
    "tage urlaub",
    "urlaubstage",
    "days of vacation",
    "vacation days",
    "altersvorsorge",
    "company pension",
    "pension scheme",
    "jobrad",
    "dienstwagen",
    "firmenwagen",
    "company car",
    "probezeit",
    "karriereseite",
    "career page",
    "gehaltsvorstellung",
    "salary expectation",
];
/// A staffing agency writing for a client.
pub(crate) const AGENCY_CUES: &[&str] = &[
    "personaldienstleist",
    "personalvermittl",
    "personalberatung",
    "fur unseren kunden",
    "im auftrag unseres kunden",
    "our client",
    "on behalf of our client",
    "staffing",
    "recruitment agency",
];

/// Salary statements: cue words, units and bounds.
pub(crate) const SALARY_CUES: &[&str] = &[
    "gehalt",
    "salary",
    "vergutung",
    "compensation",
    "brutto",
    "einkommen",
    "p. a.",
    "p.a.",
    "per annum",
];
pub(crate) const MONTHLY_WORDS: &[&str] = &["monat", "month", "/mo"];
pub(crate) const LOWER_BOUND_WORDS: &[&str] =
    &["ab ", "from ", "starting at", "mindestens", "at least"];
/// Suffixes that multiply an amount by 1,000 (`120k`, `120 TEUR`).
pub(crate) const THOUSAND_SUFFIXES: &[&str] = &["k€", "k ", "k,", "teur", "tsd", "t€"];
/// Plausible annual salaries in EUR start here.
pub(crate) const SALARY_MIN_AMOUNT: u64 = 10_000;

/// Lines that name a work location.
pub(crate) const LOCATION_LINES: &[&str] = &[
    "standort",
    "einsatzort",
    "arbeitsort",
    "dienstort",
    "location",
    "ort:",
];
/// Words of a remote share.
pub(crate) const REMOTE_WORDS: &[&str] = &[
    "remote",
    "homeoffice",
    "home-office",
    "home office",
    "mobil",
    "work from home",
];
/// Fully remote wording for permanent roles (on top of [`FULL_REMOTE`]).
pub(crate) const REMOTE_FULL_EXTRA: &[&str] = &[
    "remote-first",
    "work from anywhere",
    "komplett remote",
    "ausschliesslich remote",
    "100 % mobil",
    "100% mobil",
];
/// Words in a location field that name no place (countries, states, work modes).
pub(crate) const LOCATION_NOISE: &[&str] = &[
    "austria",
    "baden-wurttemberg",
    "bavaria",
    "bayern",
    "brandenburg",
    "bundesweit",
    "d",
    "dach",
    "de",
    "deutschland",
    "deutschlandweit",
    "germany",
    "hessen",
    "hybrid",
    "mecklenburg-vorpommern",
    "niedersachsen",
    "nordrhein-westfalen",
    "on-site",
    "onsite",
    "ort",
    "osterreich",
    "rheinland-pfalz",
    "saarland",
    "sachsen",
    "sachsen-anhalt",
    "schleswig-holstein",
    "schweiz",
    "switzerland",
    "thuringen",
    "vor",
];
/// Larger German cities, to read on-site sentences when no location is given (sorted).
pub(crate) const GERMAN_CITIES: &[&str] = &[
    "aachen",
    "augsburg",
    "berlin",
    "bielefeld",
    "bochum",
    "bonn",
    "braunschweig",
    "bremen",
    "chemnitz",
    "dortmund",
    "dresden",
    "duisburg",
    "dusseldorf",
    "erfurt",
    "essen",
    "frankfurt",
    "freiburg",
    "hamburg",
    "hannover",
    "heidelberg",
    "ingolstadt",
    "karlsruhe",
    "kassel",
    "kiel",
    "koln",
    "leipzig",
    "mainz",
    "mannheim",
    "munchen",
    "munster",
    "nurnberg",
    "regensburg",
    "rostock",
    "stuttgart",
    "ulm",
    "wiesbaden",
    "wurzburg",
];

/// Experience statements: words and bounds.
pub(crate) const EXPERIENCE_WORDS: &[&str] = &["erfahrung", "experience", "praxis"];
/// The years refer to the whole career, not one topic.
pub(crate) const CAREER_WORDS: &[&str] = &[
    "berufserfahrung",
    "berufspraxis",
    "professional",
    "work experience",
    "relevant",
    "einschlagig",
];
/// Title words of a senior role (whole words, or word endings for `...leiter`).
pub(crate) const SENIOR_TITLES: &[&str] = &[
    "senior",
    "lead",
    "principal",
    "sme",
    "head",
    "director",
    "direktor",
    "chief",
    "cfo",
    "vp",
    "leitung",
    "leiter",
    "leiterin",
];
/// Title words of a junior role.
pub(crate) const JUNIOR_TITLES: &[&str] = &[
    "junior",
    "trainee",
    "werkstudent",
    "werkstudentin",
    "praktikant",
    "praktikantin",
    "praktikum",
    "internship",
    "absolvent",
    "absolventin",
    "graduate",
    "berufseinsteiger",
    "berufseinsteigerin",
    "entry level",
    "entry-level",
];

/// Closing lines of an ad: they end a requirement section.
pub(crate) const CLOSING_WORDS: &[&str] = &[
    "interessiert?",
    "freuen wir uns auf",
    "freuen uns auf",
    "bitte beachten sie",
    "we look forward",
    "jetzt bewerben",
    "apply now",
];
/// Bullet glyphs of pasted and LinkedIn ads on top of the old engine's bullets (the new
/// engine only; the old one is frozen).
pub(crate) const EXTRA_BULLETS: &[char] = &[
    '▪', '■', '●', '◦', '✅', '✔', '✓', '➡', '→', '👉', '➤', '\u{fe0f}',
];
