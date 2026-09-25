//! Word lists of the new engine.
//!
//! external contract - do not translate: German and English wording of job ads and
//! consultant profiles. All entries are case-folded; sorted tables are searched with
//! `binary_search`, keep them sorted.

/// Words that carry no skill on top of the old stopwords (sorted).
pub(crate) const FILLERS: &[&str] = &[
    "ability",
    "abstimmung",
    "advantage",
    "advantageous",
    "aktuelle",
    "analytische",
    "anspruchsvolle",
    "anwendung",
    "asset",
    "aufgaben",
    "ausgepragt",
    "background",
    "bereich",
    "bereichen",
    "bereits",
    "besten",
    "comparable",
    "deep",
    "delivered",
    "demonstrated",
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
    "hilfreich",
    "hohe",
    "hoher",
    "idealerweise",
    "inkl",
    "insbesondere",
    "jahre",
    "jahren",
    "kenntnissen",
    "large",
    "least",
    "level",
    "mind",
    "mindestens",
    "moderne",
    "more",
    "nachweisbare",
    "nachweislich",
    "nice",
    "niveau",
    "plus",
    "preferred",
    "proven",
    "record",
    "several",
    "sicher",
    "solid",
    "sound",
    "starke",
    "strong",
    "successful",
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
    "vorteil",
    "vorzugsweise",
    "weitreichende",
    "within",
    "worked",
    "working",
    "wunschenswert",
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
    "chief",
    "consult",
    "consultant",
    "consulting",
    "digital",
    "director",
    "direktor",
    "direktorin",
    "einfuhrung",
    "erp",
    "financ",
    "finance",
    "finanz",
    "head",
    "it",
    "leitung",
    "manag",
    "management",
    "manager",
    "offic",
    "partn",
    "partner",
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
    "unternehm",
    "unternehmen",
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

/// Heads that name nearly the same skill (half a match, both ways, also at the start of a
/// compound).
pub(crate) const EQUIVALENT_HEADS: &[(&str, &str)] = &[
    ("projektleitung", "projektmanagement"),
    ("programmleitung", "programmmanagement"),
];
/// Gender endings of a word (after folding).
pub(crate) const GENDER_FORMS: &[&str] = &[
    ":innen", "*innen", "_innen", ":in", "*in", "_in", "/in", "(in)",
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
    "tschechisch",
    "ungarisch",
    "rumanisch",
    "schwedisch",
    "danisch",
    "norwegisch",
    "finnisch",
    "griechisch",
    "japanisch",
    "arabisch",
    "koreanisch",
    "kroatisch",
];
/// English names of languages (whole words) and their stem in `LANGUAGES`.
pub(crate) const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("german", "deutsch"),
    ("english", "englisch"),
    ("french", "franzosisch"),
    ("spanish", "spanisch"),
    ("italian", "italienisch"),
    ("dutch", "niederlandisch"),
    ("flemish", "niederlandisch"),
    ("polish", "polnisch"),
    ("russian", "russisch"),
    ("chinese", "chinesisch"),
    ("mandarin", "chinesisch"),
    ("portuguese", "portugiesisch"),
    ("turkish", "turkisch"),
    ("czech", "tschechisch"),
    ("hungarian", "ungarisch"),
    ("romanian", "rumanisch"),
    ("swedish", "schwedisch"),
    ("danish", "danisch"),
    ("norwegian", "norwegisch"),
    ("finnish", "finnisch"),
    ("greek", "griechisch"),
    ("japanese", "japanisch"),
    ("arabic", "arabisch"),
    ("korean", "koreanisch"),
    ("croatian", "kroatisch"),
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
    "analytical",
    "analytisch",
    "auftreten",
    "belastbar",
    "communication",
    "durchsetzung",
    "durchsetzungsstark",
    "eigeninitiative",
    "empathie",
    "engagement",
    "flexibilit",
    "freude",
    "gelassen",
    "gespur",
    "hands-on-mental",
    "humor",
    "kommunikation",
    "konfliktfahig",
    "kundenorientier",
    "losungsorientier",
    "mentalitat",
    "mentality",
    "motivation",
    "neugier",
    "organisationstalent",
    "pragmatisch",
    "prasentationsfahig",
    "prasentationsstark",
    "proaktiv",
    "reliable",
    "selbststandig",
    "sorgfalt",
    "sozialkompetenz",
    "structured",
    "strukturiert",
    "teamfahig",
    "teamplayer",
    "uberzeugungskraft",
    "verbindlich",
    "verhandlungsgeschick",
    "verhandlungsstark",
    "wertschatz",
    "zahlenaffin",
    "zuhor",
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

/// Endings of a frame or soft word that keep its meaning (`Verfügbarkeit`, `Reisen`,
/// `analytische`, `Flexibilität`) (sorted).
pub(crate) const WORD_ENDINGS: &[&str] = &[
    "at", "de", "e", "em", "en", "end", "er", "es", "heit", "ig", "ige", "igen", "iger", "iges",
    "igkeit", "ing", "itat", "keit", "keiten", "ly", "n", "s", "t", "te", "ten", "ter", "ung",
    "ungen", "voll",
];
/// Linking letters between the parts of a compound (`Gehalt-s-vorstellung`).
pub(crate) const LINKERS: &[&str] = &["es", "keits", "n", "s", "ungs"];
/// Heads that keep a frame word a frame (`Reisebereitschaft`, `Startdatum`,
/// `Gehaltsvorstellung`, `Remote-Arbeit`) (sorted).
pub(crate) const FRAME_HEADS: &[&str] = &[
    "aktivitat",
    "anteil",
    "arbeit",
    "basis",
    "bereit",
    "date",
    "datum",
    "days",
    "dienst",
    "expectation",
    "expectations",
    "first",
    "freudig",
    "freudigkeit",
    "klasse",
    "model",
    "modell",
    "option",
    "pflicht",
    "presence",
    "quote",
    "range",
    "requirement",
    "requirements",
    "share",
    "tag",
    "tage",
    "tatigkeit",
    "termin",
    "time",
    "vorstellung",
    "work",
    "working",
    "zeit",
    "zeiten",
    "zeitpunkt",
    "zeitraum",
];
/// Skill heads after an English frame word: the frame word is then a modifier
/// (`Hybrid Cloud`, `Travel Management`, `Salary Benchmarking`) (sorted).
pub(crate) const FRAME_MODIFIED_HEADS: &[&str] = &[
    "accounting",
    "administration",
    "analysis",
    "analytics",
    "architecture",
    "audit",
    "audits",
    "banding",
    "bands",
    "benchmark",
    "benchmarking",
    "cloud",
    "compliance",
    "controlling",
    "design",
    "governance",
    "infrastructure",
    "integration",
    "management",
    "manager",
    "migration",
    "modeling",
    "modelling",
    "monitoring",
    "negotiation",
    "negotiations",
    "operations",
    "optimisation",
    "optimization",
    "planning",
    "policies",
    "policy",
    "process",
    "processes",
    "program",
    "programme",
    "reporting",
    "review",
    "reviews",
    "services",
    "solution",
    "solutions",
    "sourcing",
    "strategy",
    "structure",
    "structures",
    "survey",
    "surveys",
    "system",
    "systems",
];
/// Modifiers that make a compound ending in a frame word a skill (`Online-Präsenz`).
pub(crate) const NOT_FRAME_MODIFIERS: &[&str] = &[
    "brand", "digital", "internet", "markt", "medien", "online", "social", "web",
];
/// An item naming compensation work is a skill, not a frame (`Vergütung und Benefits`).
pub(crate) const FRAME_SKILL_CONTEXT: &[&str] = &[
    "benefit",
    "compensation",
    "entgelt",
    "grading",
    "payroll",
    "reward",
    "tarif",
];
/// Heads that keep a soft word soft (`Kommunikationsfähigkeit`, `analytisches Denken`)
/// (sorted).
pub(crate) const SOFT_HEADS: &[&str] = &[
    "approach",
    "arbeiten",
    "arbeitsweise",
    "art",
    "attitude",
    "auftreten",
    "denken",
    "denkvermogen",
    "denkweise",
    "fahigkeit",
    "fahigkeiten",
    "geschick",
    "kompetenz",
    "kompetenzen",
    "manner",
    "mindset",
    "personlichkeit",
    "skill",
    "skills",
    "starke",
    "starken",
    "style",
    "talent",
    "thinking",
    "vermogen",
    "vorgehen",
    "vorgehensweise",
];

/// Short tokens that name a skill or role (`QP`, `R`, `Go`, `5S`, `8D`, `IQ/OQ/PQ`, `CI/CD`)
/// and survive the minimum length (sorted).
pub(crate) const SHORT_TOKENS: &[&str] = &[
    "5s", "8d", "ai", "bi", "cd", "ci", "go", "hr", "iq", "ml", "oq", "pq", "qa", "qc", "qp", "r",
    "ux", "vp",
];
/// Codes that a number completes (`ISO 9001`, `Annex 11`, `IEC 62304`) (sorted).
pub(crate) const NUMBERED_CODES: &[&str] = &["anhang", "annex", "din", "iec", "iso", "part"];
/// Parts after a hyphenated skill that only say "knowledge of" (`SQL-Kenntnisse`,
/// `CAPA-Erfahrung`, `SAP-Know-how`).
pub(crate) const KNOWLEDGE_SUFFIXES: &[&str] = &[
    "-erfahrung",
    "-erfahrungen",
    "-expertise",
    "-kenntnis",
    "-kenntnisse",
    "-kenntnissen",
    "-know-how",
    "-knowledge",
    "-wissen",
];
/// Endings of an adjective that can share the noun of the next item
/// (`Classic and agile project management`, `klassische und agile Methoden`).
pub(crate) const ADJECTIVE_ENDINGS: &[&str] = &[
    "al", "ale", "alen", "aler", "ales", "ic", "ical", "isch", "ische", "ischen", "ischer",
    "isches", "iv", "ive", "iven", "iver", "ives", "lich", "liche", "lichen", "licher", "liches",
];
/// Endings of a declined adjective (`großer`, `externen`, `neues`).
pub(crate) const DECLINED_ENDINGS: &[&str] = &["er", "en", "es", "em"];
/// Words for days on site in a frame line (`2 Tage vor Ort`).
pub(crate) const DAY_WORDS_ONSITE: &[&str] = &[
    "tag", "tage", "tagen", "day", "days", "woche", "week", "vor", "ort", "on-site", "onsite",
    "hybrid", "remote", "prasenz", "buro", "office",
];
/// Verbal particles: `Einführung`, `Durchführung`, `Ausbildung` are no compounds of
/// `Führung` or `Bildung`; `Buchführung` is bookkeeping, no leadership.
/// Heads that verbal particles bind to (`Einführung`, `Durchführung`, `Markteinführung`): a
/// modifier ending in a particle makes another word, no compound of the head.
pub(crate) const PARTICLE_HEADS: &[&str] = &["fuhrung"];
pub(crate) const PARTICLE_MODIFIERS: &[&str] = &[
    "ab", "an", "auf", "aus", "bei", "buch", "durch", "ein", "ent", "fort", "mit", "nach", "uber",
    "um", "unter", "ver", "vor", "weg", "wieder", "zu", "zuruck",
];
/// Leadership asked for (`Führungserfahrung`, `leadership experience`): a profile with a
/// leading role meets it at least half.
pub(crate) const LEADERSHIP_ATOMS: &[&str] = &[
    "disziplinarisch",
    "fuhrung",
    "fuhrungserfahrung",
    "fuhrungskompetenz",
    "fuhrungsverantwortung",
    "leadership",
    "mitarbeiterfuhrung",
    "personalfuhrung",
    "personalverantwortung",
    "teamfuhrung",
    "teamleitung",
];
/// Words of a leading role in a profile entry (`Leiter Controlling`, `Head of IT`, `CFO`).
pub(crate) const PROFILE_LEAD_WORDS: &[&str] = &[
    "leiter",
    "leiterin",
    "leitung",
    "head",
    "director",
    "direktor",
    "chief",
    "cfo",
    "cio",
    "coo",
    "ceo",
    "cto",
    "geschaftsfuhrer",
    "geschaftsfuhrung",
    "vorstand",
    "werksleiter",
    "vp",
    "teamleiter",
    "abteilungsleiter",
    "bereichsleiter",
];

/// Stems that ask for knowledge (`Kenntnisse`, `Erfahrung`, `know-how`).
pub(crate) const KNOWLEDGE_STEMS: &[&str] = &[
    "kenntnis",
    "erfahrung",
    "knowledge",
    "experience",
    "know-how",
    "wissen",
    "skills",
];
/// Words that only say "knowledge of" around a skill (`SAP-Kenntnisse`, `Erfahrung mit SAP`).
pub(crate) const KNOWLEDGE_WORDS: &[&str] = &[
    "erfahrung",
    "erfahrungen",
    "experience",
    "kenntnis",
    "kenntnisse",
    "knowledge",
    "know-how",
    "skills",
    "wissen",
    "sehr",
    "gute",
    "gut",
    "fundierte",
    "umfassende",
    "tiefe",
];

/// Shortest modifier of a compound (`Bericht-erstellung`); `h` of `Herstellung` is none.
pub(crate) const MIN_COMPOUND_MODIFIER: usize = 3;

/// Gender markers in a title (`(all genders)`, `(gn)`), and the letters of `(m/w/d)`.
pub(crate) const GENDER_MARKERS: &[&str] = &[
    "all genders",
    "alle geschlechter",
    "gn",
    "gn*",
    "m/w/d",
    "w/m/d",
];
pub(crate) const GENDER_LETTERS: &[&str] = &["d", "f", "i", "m", "w", "x", "div", "divers"];
/// Separators of a title's marketing tail (`Interim CFO – Scale-up E-Mobility`).
pub(crate) const TITLE_TAIL_SEPARATORS: &[&str] = &[" – ", " — ", " | ", " - "];
/// Contract words in a title that say nothing about the field (title fit).
pub(crate) const TITLE_CONTRACT_WORDS: &[&str] = &[
    "befristet",
    "befristete",
    "befristeter",
    "contract",
    "contractor",
    "freelance",
    "freelancer",
    "freiberufler",
    "freiberuflich",
    "interim",
    "interimistisch",
    "temporary",
    "zeitlich",
];
/// Items that are only a soft word (`Arbeitsweise`, `working style`, `Soft Skills`); compared
/// as stems.
pub(crate) const SOFT_ALONE: &[&str] = &[
    "arbeitsweise",
    "eigenschaften",
    "fahigkeiten",
    "haltung",
    "kompetenzen",
    "mentalitat",
    "mindset",
    "personlichkeit",
    "skills",
    "soft",
    "style",
];
/// A sentence starting with one of these words and naming no known skill is a soft
/// requirement (`Sie kommunizieren klar`, `Du packst gerne mit an`).
pub(crate) const PRONOUN_STARTS: &[&str] =
    &["sie", "du", "you", "your", "ihr", "ihre", "dein", "deine"];
/// Phrases whose `und`/`and` joins no two requirements (`Deutsch in Wort und Schrift`).
pub(crate) const PROTECTED_PHRASES: &[&str] = &[
    "in wort und schrift",
    "wort und schrift",
    "mündlich und schriftlich",
    "schriftlich und mündlich",
    "written and spoken",
    "spoken and written",
    "written and verbal",
    "verbal and written",
];
/// Words after a comma that continue the item (`SAP, gerne auch S/4HANA`,
/// `klar, auch wenn es unbequem wird`); `, davon 3 Jahre in Führung` splits.
pub(crate) const COMMA_TAILS: &[&str] = &[
    "auch ",
    "gerne ",
    "gern ",
    "davon ",
    "ob ",
    "ohne dass",
    "auch wenn",
    "as ",
    "egal ",
    "insbesondere ",
    "vorzugsweise ",
];
/// Words after which a list of bare nouns names the partners of one item
/// (`Zusammenarbeit mit Gesellschaftern, Investoren und Dienstleistern`); a list after
/// `Erfahrung mit` names skills and splits.
pub(crate) const LIST_OBJECT_WORDS: &[&str] = &[
    "gegenüber",
    "rund um",
    "towards",
    "zusammenarbeit mit",
    "umgang mit",
    "abstimmung mit",
    "austausch mit",
    "kontakt mit",
    "kommunikation mit",
    "verhandlungen mit",
    "schnittstelle zu",
    "working with",
    "collaboration with",
    "interaction with",
    "liaising with",
    "dealing with",
    "interface with",
];

/// Formal requirements: degrees (prefix stems; a bare `Abschluss` is also a financial
/// statement, so it does not count).
pub(crate) const DEGREE_WORDS: &[&str] = &[
    "b.sc",
    "bachelor",
    "bsc",
    "degree",
    "diplom",
    "doctorate",
    "hochschulabschluss",
    "m.sc",
    "master",
    "msc",
    "ph.d",
    "phd",
    "promotion",
    "promoviert",
    "staatsexamen",
    "staatsexamina",
    "studium",
    "university",
];
/// Words that make `Promotion` sales work, not a doctorate (`Sales Promotion`).
pub(crate) const PROMOTION_NOT_DEGREE: &[&str] = &[
    "handel",
    "marketing",
    "sales",
    "trade",
    "verkauf",
    "vertrieb",
];
/// Words after `master` that make it no degree (`Master Data Management`).
pub(crate) const MASTER_NOT_DEGREE: &[&str] = &["data", "daten", "file", "plan", "record"];

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
    ("natural science", "science"),
    ("rer. nat", "science"),
    ("rer.nat", "science"),
    ("pharmaz", "life-science"),
    ("pharmacy", "life-science"),
    ("pharmaceutical science", "life-science"),
    ("apothek", "life-science"),
    ("chemie", "life-science"),
    ("chemistry", "life-science"),
    ("biolog", "life-science"),
    ("biochem", "life-science"),
    ("biotechnolog", "life-science"),
    ("life science", "life-science"),
    ("lebenswissenschaft", "life-science"),
    // Fields of the other domain packs (data, HR, operations, software, marketing).
    ("statistik", "science"),
    ("statistics", "science"),
    ("medizin", "medicine"),
    ("medicine", "medicine"),
    ("medizintechnik", "engineering"),
    ("mechatronik", "engineering"),
    ("produktionstechnik", "engineering"),
    ("fertigungstechnik", "engineering"),
    ("fahrzeugtechnik", "engineering"),
    ("psycholog", "psychology"),
    ("softwaretechnik", "it"),
    ("software engineering", "it"),
    ("data science", "it"),
    ("marketing", "business"),
];

/// Neighbouring degree fields: a degree in one half-meets a requirement for the other.
pub(crate) const DEGREE_RELATED: &[(&str, &str)] = &[
    ("science", "life-science"),
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
    ("staatsexamina", 2),
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
    "qualified person",
    "sachkundige person",
    "sachkundigen person",
];
/// Names of the same licence: a profile holding one meets a requirement for another.
pub(crate) const LICENCE_SYNONYMS: &[&[&str]] = &[&[
    "qualified person",
    "sachkundige person",
    "sachkundigen person",
    "sachkundiger person",
    "qp",
]];
/// A licence word counts only as a qualification (`Zulassung als ...`, `... examen`).
pub(crate) const LICENCE_CONTEXT: &[&str] = &[
    "amg",
    "(qp)",
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
/// Words that open a part of the years a line stated before them (`Mehrjährige Erfahrung,
/// davon mindestens drei Jahre in ...`): the years after them are no minimum of the ad.
pub(crate) const YEARS_SUBSPAN: &[&str] = &["davon", "hiervon", "darunter", "thereof", "including"];
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
/// English keys of the language list, its entries and their level.
pub(crate) const KEYS_LANGUAGES: &[&str] = &[KEY_LANGUAGES, "languages"];
pub(crate) const KEYS_LANGUAGE: &[&str] = &[KEY_LANGUAGE, "language", "name"];
pub(crate) const KEYS_LEVEL: &[&str] = &[KEY_LEVEL, "level"];
pub(crate) const KEYS_YEARS: &[&str] = &["jahre", "years", "erfahrung_jahre"];
/// Free-text USPs (`alleinstellungsmerkmale`).
pub(crate) const KEY_USP: &str = "alleinstellungsmerkmal";
/// Career stations (`stationen`): their bullets are free text, weaker evidence like a USP.
pub(crate) const KEY_STATIONS: &str = "stationen";
/// Industries of the profile (`branchen`): the setting of a requirement, never its function.
pub(crate) const KEY_INDUSTRY_LIST: &str = "branchen";
/// Profile atoms that say nothing about the field in the relevance query: contract words
/// written as one word and quantities (the number words count as well).
pub(crate) const QUERY_NOISE: &[&str] = &["davon", "elf", "interimmanagement", "jeden", "rund"];
/// Headings that are a portal's tag list, not the ad's requirements (whole heading).
pub(crate) const TAG_HEADINGS: &[&str] = &["skills"];

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
    "muss-anforderung",
    "mussanforderung",
    "muss-kriteri",
    "musskriteri",
];
/// Extra nice headings.
pub(crate) const NICE_PREFIXES: &[&str] = &[
    "preferred",
    "nice to have",
    "nice-to-have",
    "good to have",
    "a plus",
    "soll-anforderung",
    "sollanforderung",
    "soll-kriteri",
    "sollkriteri",
    "kann-anforderung",
    "kannanforderung",
    "kann-kriteri",
    "kannkriteri",
];
/// Headings (folded) of the other listings a portal shows under an ad: nothing below them
/// belongs to the ad, so no hard criterion (ANÜ, country, rate, permanent role, seniority)
/// is read from there.
pub(crate) const OTHER_LISTINGS: &[&str] = &[
    "ahnliche projekte",
    "ahnliche jobs",
    "ahnliche stellen",
    "ahnliche stellenangebote",
    "weitere projekte",
    "weitere jobs",
    "weitere stellen",
    "andere projekte",
    "similar projects",
    "similar jobs",
    "more jobs",
    "people also viewed",
];
/// What may follow a heading of `OTHER_LISTINGS` on its line (folded): a portal's link
/// text or the listings' owner.
pub(crate) const LISTING_TAILS: &[&str] = &[
    "anzeigen",
    "dieses anbieters",
    "des anbieters",
    "fur dich",
    "for you",
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
    "terms",
    "rahmen",
    "weitere infos",
    "weitere informationen",
    "kategorien",
    "tags",
    // Portal chrome after the ad (the provider, more projects).
    "projektanbieter",
    "weitere projekte",
    "ahnliche projekte",
    "similar projects",
    "similar jobs",
    "ahnliche jobs",
    // Portal footers and meta lines.
    "projekt-id",
    "projekt id",
    "projektnummer",
    "projekt-nr",
    "referenznummer",
    "kennziffer",
    "job-id",
    "eingestellt am",
    "veroffentlicht am",
    "online seit",
    "branche",
    "kategorie",
    "karrierestufe",
    "beschaftigungsverhaltnis",
    "tatigkeitsbereich",
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
    "ein plus",
    "strong plus",
    "gerne mit",
    "an advantage",
    "advantageous",
    "hilfreich",
    "grosses plus",
    "an asset",
    "klarer vorteil",
    "preferred",
    "is a plus",
];
/// Nice cues that close a line (`X und Y von Vorteil`): the whole line is nice.
pub(crate) const NICE_CLOSING: &[&str] = &[
    "von vorteil",
    "wunschenswert",
    "a plus",
    "ein plus",
    "strong plus",
    "nice to have",
    "nice-to-have",
    "vorteilhaft",
    "bonus",
    "an advantage",
    "advantageous",
    "hilfreich",
    "grosses plus",
    "an asset",
    "klarer vorteil",
    "preferred",
    "is a plus",
];
/// A requirement item that says something is not needed.
pub(crate) const NOT_NEEDED: &[&str] = &[
    "nicht notwendig",
    "nicht erforderlich",
    "nicht notig",
    "nicht zwingend",
    "keine voraussetzung",
    "not required",
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
    "approx", "bzw", "ca", "dr", "evtl", "ggf", "inkl", "max", "min", "mind", "nr", "vgl", "zzgl",
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
pub(crate) const SALARY_WORDS: &[&str] = &[
    "gehalt",
    "salary",
    "p.a.",
    "t€",
    "per annum",
    "pro jahr",
    "per year",
    "jahresbrutto",
];
/// Labels of the place of work in a frame line (`Ort: 3199 Rotterdam, Niederlande`).
pub(crate) const PLACE_LABELS: &[&str] = &[
    "ort:",
    "standort:",
    "arbeitsort:",
    "einsatzort:",
    "dienstort:",
    "location:",
    "place of work:",
    "work location:",
    "job location:",
    "office location:",
];
/// A sentence that denies a contract form gives no contract signal
/// (`Interim Management oder Arbeitnehmerüberlassung ist nicht vorgesehen`).
pub(crate) const CONTRACT_DENIED: &[&str] = &[
    "nicht vorgesehen",
    "nicht moglich",
    "nicht gewunscht",
    "nicht gewollt",
    "ausgeschlossen",
    "not possible",
    "not an option",
    "not considered",
    "kein interim",
    "keine interim",
    "keine freelancer",
    "keine freiberufler",
    "no freelancer",
    "no interim",
    "not consider",
    "don't consider",
];
/// Currencies and rate units next to an amount (`950 €`, `EUR 950`, `95 €/h`).
pub(crate) const RATE_UNITS: &[&str] = &[
    "€",
    "eur",
    "euro",
    "chf",
    "usd",
    "gbp",
    "$",
    "£",
    "/h",
    "/std",
    "/tag",
    "/day",
    "pro tag",
    "pro stunde",
    "per day",
    "per hour",
    "k€",
];
/// Words of a rate range (`bis 1.100 €`, `ab 900`, `max. 1.000`).
pub(crate) const RATE_RANGE_WORDS: &[&str] = &[
    "maximal", "circa", "up to", "from", "rund", "max.", "max", "ca.", "ca", "bis", "von", "ab",
    "to", "zu",
];
/// Separators of the parts of one line (`Start: 02/2027 · Dauer: 10 Monate · 78 €/h`).
pub(crate) const SEGMENT_SEPARATORS: &[&str] = &[" // ", " · ", " | ", " • "];
/// A rate to be agreed, without an amount (with a rate word in the same sentence).
pub(crate) const RATE_OPEN: &[&str] = &[
    "nach absprache",
    "nach vereinbarung",
    "auf anfrage",
    "verhandelbar",
    "verhandlungssache",
    "negotiable",
    "on request",
    "to be agreed",
    "tbd",
];
/// Sentences that state a duration, and the units of one.
pub(crate) const DURATION_WORDS: &[&str] = &[
    "laufzeit",
    "dauer",
    "duration",
    "zeitraum",
    "einsatzzeitraum",
    "length",
];
pub(crate) const MONTH_UNITS: &[&str] = &["monat", "month"];
pub(crate) const WEEK_UNITS: &[&str] = &["woche", "week"];
pub(crate) const HOURLY_WORDS: &[&str] = &["stunde", "std", "hour", "/h", "stundensatz"];
/// A day rate named in the value of a rate line.
pub(crate) const DAILY_WORDS: &[&str] = &[
    "tagessatz",
    "pro tag",
    "per day",
    "/tag",
    "/day",
    "day rate",
    "daily rate",
];
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
    ("aalborg", "DK"),
    ("aarau", "CH"),
    ("aarhus", "DK"),
    ("amersfoort", "NL"),
    ("amsterdam", "NL"),
    ("antwerp", "BE"),
    ("antwerpen", "BE"),
    ("arnhem", "NL"),
    ("atlanta", "US"),
    ("bangalore", "IN"),
    ("barcelona", "ES"),
    ("basel", "CH"),
    ("bergamo", "IT"),
    ("bergen", "NO"),
    ("bern", "CH"),
    ("biel", "CH"),
    ("bilbao", "ES"),
    ("birmingham", "GB"),
    ("bologna", "IT"),
    ("bolzano", "IT"),
    ("bordeaux", "FR"),
    ("boston", "US"),
    ("bozen", "IT"),
    ("bratislava", "SK"),
    ("breda", "NL"),
    ("bregenz", "AT"),
    ("brescia", "IT"),
    ("breslau", "PL"),
    ("brno", "CZ"),
    ("brugge", "BE"),
    ("brunn", "CZ"),
    ("brussel", "BE"),
    ("brussels", "BE"),
    ("bruxelles", "BE"),
    ("bucharest", "RO"),
    ("budapest", "HU"),
    ("budweis", "CZ"),
    ("bukarest", "RO"),
    ("bydgoszcz", "PL"),
    ("ceske budejovice", "CZ"),
    ("charleroi", "BE"),
    ("chicago", "US"),
    ("chur", "CH"),
    ("cluj-napoca", "RO"),
    ("copenhagen", "DK"),
    ("cork", "IE"),
    ("cracow", "PL"),
    ("danzig", "PL"),
    ("debrecen", "HU"),
    ("delft", "NL"),
    ("den haag", "NL"),
    ("dornbirn", "AT"),
    ("dublin", "IE"),
    ("edinburgh", "GB"),
    ("eindhoven", "NL"),
    ("florence", "IT"),
    ("florenz", "IT"),
    ("gdansk", "PL"),
    ("gdynia", "PL"),
    ("geneva", "CH"),
    ("geneve", "CH"),
    ("genf", "CH"),
    ("genoa", "IT"),
    ("gent", "BE"),
    ("genua", "IT"),
    ("ghent", "BE"),
    ("glasgow", "GB"),
    ("goteborg", "SE"),
    ("gothenburg", "SE"),
    ("graz", "AT"),
    ("grenoble", "FR"),
    ("groningen", "NL"),
    ("haarlem", "NL"),
    ("helsinki", "FI"),
    ("houston", "US"),
    ("innsbruck", "AT"),
    ("katowice", "PL"),
    ("kattowitz", "PL"),
    ("klagenfurt", "AT"),
    ("kobenhavn", "DK"),
    ("kopenhagen", "DK"),
    ("kosice", "SK"),
    ("krakau", "PL"),
    ("krakow", "PL"),
    ("lausanne", "CH"),
    ("leeds", "GB"),
    ("leiden", "NL"),
    ("leoben", "AT"),
    ("leuven", "BE"),
    ("liberec", "CZ"),
    ("liege", "BE"),
    ("lille", "FR"),
    ("linz", "AT"),
    ("lisbon", "PT"),
    ("lissabon", "PT"),
    ("ljubljana", "SI"),
    ("lodz", "PL"),
    ("london", "GB"),
    ("los angeles", "US"),
    ("lublin", "PL"),
    ("lucerne", "CH"),
    ("lugano", "CH"),
    ("luttich", "BE"),
    ("luxembourg", "LU"),
    ("luxemburg", "LU"),
    ("luzern", "CH"),
    ("lyon", "FR"),
    ("maastricht", "NL"),
    ("madrid", "ES"),
    ("mailand", "IT"),
    ("malmo", "SE"),
    ("manchester", "GB"),
    ("marseille", "FR"),
    ("mechelen", "BE"),
    ("metz", "FR"),
    ("milan", "IT"),
    ("milano", "IT"),
    ("montpellier", "FR"),
    ("mulhausen", "FR"),
    ("mulhouse", "FR"),
    ("nantes", "FR"),
    ("naples", "IT"),
    ("neapel", "IT"),
    ("neuchatel", "CH"),
    ("new york", "US"),
    ("nijmegen", "NL"),
    ("odense", "DK"),
    ("olomouc", "CZ"),
    ("olten", "CH"),
    ("oslo", "NO"),
    ("ostrava", "CZ"),
    ("padova", "IT"),
    ("padua", "IT"),
    ("paris", "FR"),
    ("pilsen", "CZ"),
    ("plzen", "CZ"),
    ("porto", "PT"),
    ("posen", "PL"),
    ("poznan", "PL"),
    ("prag", "CZ"),
    ("prague", "CZ"),
    ("praha", "CZ"),
    ("pressburg", "SK"),
    ("pune", "IN"),
    ("rennes", "FR"),
    ("rom", "IT"),
    ("roma", "IT"),
    ("rome", "IT"),
    ("rotterdam", "NL"),
    ("salzburg", "AT"),
    ("san francisco", "US"),
    ("schaffhausen", "CH"),
    ("seattle", "US"),
    ("sevilla", "ES"),
    ("st. gallen", "CH"),
    ("st. polten", "AT"),
    ("stettin", "PL"),
    ("steyr", "AT"),
    ("stockholm", "SE"),
    ("strasbourg", "FR"),
    ("strassburg", "FR"),
    ("szczecin", "PL"),
    ("the hague", "NL"),
    ("thun", "CH"),
    ("tilburg", "NL"),
    ("torino", "IT"),
    ("toulouse", "FR"),
    ("trento", "IT"),
    ("trient", "IT"),
    ("turin", "IT"),
    ("utrecht", "NL"),
    ("valencia", "ES"),
    ("venedig", "IT"),
    ("venice", "IT"),
    ("venlo", "NL"),
    ("verona", "IT"),
    ("villach", "AT"),
    ("warsaw", "PL"),
    ("warschau", "PL"),
    ("wels", "AT"),
    ("wien", "AT"),
    ("wiener neustadt", "AT"),
    ("winterthur", "CH"),
    ("wroclaw", "PL"),
    ("zagreb", "HR"),
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
/// Prepositions of a requirement part that names an activity (`im Aufbau`, `in der Führung`)
/// and the words that open its object (`von Vertriebsteams`): parts joined by `und` share
/// the object the last one names.
pub(crate) const ACTIVITY_PREPOSITIONS: &[&str] = &["im", "in", "beim", "bei", "zur", "zum", "am"];
pub(crate) const OBJECT_OPENERS: &[&str] = &["von", "of"];
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
pub(crate) const ANUE_NEGATION: &[&str] = &[
    "kein", "keine", "nicht", "ohne", "not", "no", "without", "never",
];
pub(crate) const ANUE_NEGATION_PARTS: &[&str] = &[
    "ausgeschlossen",
    "abgrenzung",
    "nicht vorgesehen",
    "not considered",
    "excluded",
    "ruled out",
];
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
    // LinkedIn's salary chip (`72.000 €/Jahr - 88.000 €/Jahr`).
    "€/jahr",
    "eur/jahr",
    "€/yr",
    "/yr",
];
/// A student or trainee role in the title is employment, never interim work.
pub(crate) const STUDENT_ROLES: &[&str] = &[
    "werkstudent",
    "werkstudierende",
    "praktikant",
    "praktikum",
    "pflichtpraktikum",
    "working student",
    "internship",
    "intern ",
    "trainee",
    "auszubildende",
    "ausbildung zum",
    "ausbildung zur",
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
/// Values of a page's employment type field (exact, folded) that mean a limited engagement:
/// LinkedIn's "Befristet", "Contract", "Temporary", the freelance portals' "Freiberuflich".
pub(crate) const LIMITED_CONTRACT_VALUES: &[&str] = &[
    "befristet",
    "contract",
    "temporary",
    "temporar",
    "freiberuflich",
    "freelance",
    "selbststandig",
    "self-employed",
];
/// Values of a page's career level or employment type field (exact, folded) that are
/// clearly below a senior target: an internship, an entry-level role, voluntary work.
pub(crate) const ENTRY_LEVEL_VALUES: &[&str] = &[
    "praktikum",
    "internship",
    "berufseinstieg",
    "einstiegslevel",
    "entry level",
    "entry-level",
    "ehrenamtlich",
    "volunteer",
    "werkstudent",
    "trainee",
];
/// Career levels (exact, folded) that may be below a senior target: a check only.
pub(crate) const LOW_LEVEL_VALUES: &[&str] = &["assistent", "assistant", "associate", "junior"];
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
    // LinkedIn's salary chip.
    "€/jahr",
    "eur/jahr",
    "/yr",
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
/// A minimum of years without the word experience (`Min. 5 years in ...`).
/// `5+ years` and `5+ Jahre` are minimums too.
pub(crate) const MIN_MARKERS: &[&str] = &[
    "min.",
    "mind.",
    "mindestens",
    "at least",
    "minimum",
    "+ year",
    "+ jahr",
];
/// A must that asks for first professional experience: an entry-level role, like a junior
/// title (whole words or phrases; `graduate` alone is no signal, `graduate degree` is a
/// master's).
pub(crate) const ENTRY_LEVEL_MUSTS: &[&str] = &[
    "erste berufserfahrung",
    "erste berufserfahrungen",
    "ersten berufserfahrungen",
    "erster berufserfahrung",
    "first professional experience",
    "first work experience",
    "berufseinsteiger",
    "berufseinsteigerin",
    "berufseinsteigende",
    "berufseinstieg",
    "absolvent",
    "absolventin",
    "absolventen",
    "hochschulabsolvent",
    "hochschulabsolventin",
    "hochschulabsolventen",
    "recent graduate",
    "recent graduates",
    "graduate program",
    "graduate programme",
    "werkstudententatigkeit",
];
/// Words of a variable pay next to a salary (`plus bis zu 20 % Bonus`).
pub(crate) const BONUS_WORDS: &[&str] = &["bonus", "variab", "tantieme"];
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
    "berufseinstieg",
    "young professional",
    "entry level",
    "entry-level",
];

/// Closing lines of an ad: they end a requirement section.
/// Lines that end requirements from their start on (portal tags, notices, application and
/// privacy text); a requirement may name these words later in the line.
pub(crate) const CLOSING_STARTS: &[&str] = &[
    "skills:",
    "tags:",
    "kategorien:",
    "kategorie:",
    "keywords:",
    "schlagworte:",
    "hinweis",
    "please note",
    "bitte senden sie",
    "wir melden uns",
    "datenschutz",
];
/// Phrases that make an item soft (`Freude an Zahlen`, `Leadership style`, `build teams`).
pub(crate) const SOFT_PHRASES: &[&str] = &[
    "freude an",
    "spass an",
    "interesse an",
    "interest in",
    "confident when presenting",
    "presentation skills",
    "leadership style",
    "fuhrungsstil",
    "build teams",
    "hands-on mentality",
    "hands-on mentalitat",
];
/// Legal forms of a company: a line naming one is about the company, no requirement.
pub(crate) const LEGAL_FORMS: &[&str] = &[
    "gmbh",
    "ag",
    "se",
    "kg",
    "kgaa",
    "ohg",
    "ltd",
    "inc",
    "llc",
    "plc",
    "gmbh & co. kg",
    "b.v.",
    "s.a.",
    "sarl",
    "s.r.l.",
];
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
