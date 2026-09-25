//! Frozen parameters of the new engine (change them only with a new `ENGINE_VERSION`).

/// Evidence of an item in per-mille.
pub(crate) const E_FULL: u16 = 1000;
pub(crate) const E_HALF: u16 = 500;
pub(crate) const E_NONE: u16 = 0;

/// Weight of a must item (skill, language, degree, years).
pub(crate) const W_MUST: u64 = 1000;
/// Weight of a soft skill; unproven soft skills count half.
pub(crate) const W_SOFT: u64 = 250;
/// Weight of a vocabulary term (no explicit requirements).
pub(crate) const W_TERM: u64 = 250;
/// Evidence one nice-to-have adds.
pub(crate) const N_NICE: u64 = 500;

/// Shrinkage towards the relevance: `P' = (n*P + k*R) / (n + k)`.
pub(crate) const K_SHRINK: u64 = 2000;
/// Low evidence (teaser, vocabulary only, nice-to-haves only, fewer than two items) is also
/// shrunk towards a cautious prior.
pub(crate) const LOW_PRIOR: u64 = 400;
pub(crate) const LOW_PRIOR_WEIGHT: u64 = 1500;

/// BM25F-like relevance: k1 and b in per-mille, fixed document length in characters.
pub(crate) const BM25_K1: u64 = 1200;
pub(crate) const BM25_B: u64 = 750;
pub(crate) const BM25_LENGTH: u64 = 2400;
/// Field weights: title, requirement lines, rest.
pub(crate) const FIELD_TITLE: u64 = 3;
pub(crate) const FIELD_REQUIREMENTS: u64 = 2;
pub(crate) const FIELD_REST: u64 = 1;
/// Static specificity of a profile atom: specific and generic.
pub(crate) const SPECIFIC_WEIGHT: u64 = 1000;
pub(crate) const GENERIC_WEIGHT: u64 = 200;
/// Relevance mass at which the lexical relevance reaches one half.
pub(crate) const RELEVANCE_HALF: u64 = 7000;

/// Texts shorter than this (characters, stripped) cannot be scored.
pub(crate) const MIN_TEXT_CHARS: usize = 100;
/// Fewer assessable items than this is low evidence.
pub(crate) const LOW_EVIDENCE_ITEMS: usize = 2;
/// Hours per day for hourly rates.
pub(crate) const HOURS_PER_DAY: u64 = 8;
/// A profile entry with this many specific atoms is free text: containing a requirement
/// proves nothing (USP sentences never do).
pub(crate) const SENTENCE_ATOMS: usize = 4;
/// Permanent roles are the second category of a consultant: the shrunk fit is scaled
/// by this factor (per-mille).
pub(crate) const PERMANENT_FACTOR: u64 = 900;
/// Score caps of the rubric (1-10 scale times ten): an open formal must (degree field,
/// licence) at most 4; several musts open (at least two and at least half) at most 4; an
/// open must on the topic of the title (the core of the role) at most 6; no skill must
/// met at all (at least two) is outside the field.
pub(crate) const FORMAL_CAP: u8 = 40;
pub(crate) const SEVERAL_OPEN_CAP: u8 = 40;
pub(crate) const TITLE_OPEN_CAP: u8 = 60;
pub(crate) const OFF_FIELD_CAP: u8 = 25;
/// A scored ad keeps at least this (the rubric never shows a 1 of 10).
pub(crate) const SCORE_FLOOR: u8 = 10;

// --- ENGINE_VERSION 4: Schwerpunkte, target roles and wishes (all off without their key) ---

/// Schwerpunkte that count (profiles name three to five; more are cut with a warning).
pub(crate) const FOCUS_MAX: usize = 5;
/// A requirement met in full through a Schwerpunkt weighs this many times in `M` and `K`;
/// the evidence `n` stays as it was, so the shrinkage does not change.
pub(crate) const FOCUS_FACTOR: u64 = 2;
/// Relevance (per-mille) a Schwerpunkt demanded in the title or a requirement adds, for at
/// most `FOCUS_RELEVANCE_MAX` Schwerpunkte (in profile order).
pub(crate) const FOCUS_RELEVANCE: u64 = 100;
pub(crate) const FOCUS_RELEVANCE_MAX: usize = 3;
/// Target roles: per-mille added to the shrunk fit when the title matches a target role in
/// full or in half (the best role counts; the caps still apply afterwards).
pub(crate) const ROLE_FULL: i64 = 80;
pub(crate) const ROLE_HALF: i64 = 40;
/// Wishes: per-mille added for a wish met and taken for a wish clearly missed; near and
/// unknown add nothing. The four together reach at most `WISH_MAX`.
pub(crate) const WISH_RATE: i64 = 30;
pub(crate) const WISH_REMOTE: i64 = 30;
pub(crate) const WISH_REGION: i64 = 20;
pub(crate) const WISH_INDUSTRY: i64 = 20;
/// Tokens of the profile a domain pack needs among its triggers to switch on.
pub(crate) const PACK_HITS: usize = 2;
/// Bound of the summed wish effect (per-mille, both directions).
pub(crate) const WISH_MAX: i64 = 100;
/// Working hours of a year (40 per week): an hourly wage of an employment per year.
pub(crate) const HOURS_PER_YEAR: u64 = 2080;
/// A single open skill must caps the score as off-field only when the title fit (per-mille)
/// stays below this: the title names little of the profile either.
pub(crate) const OFF_FIELD_TITLE_FIT: u64 = 300;
/// The cap of that single open skill must (the cap of two or more is `OFF_FIELD_CAP`).
pub(crate) const OFF_FIELD_SINGLE_CAP: u8 = 30;
/// Highest score of a text without any requirement (judged from title and words alone).
pub(crate) const NO_ITEMS_CAP: u8 = 60;
/// While fewer than half of the musts are met, the target role and the wishes lift a score
/// at most to this (per-mille, score 79): never into the high band (80) of the list.
pub(crate) const LIFT_CAP: u64 = 790;
/// A day rate of at least this share of the wish (per-mille) is near, below it missed.
pub(crate) const WISH_RATE_NEAR: u64 = 950;
/// Remote wishes as minimum remote shares (percent): `voll`, `ueberwiegend`, `teilweise`.
pub(crate) const REMOTE_FULL: u64 = 100;
pub(crate) const REMOTE_MOSTLY: u64 = 60;
pub(crate) const REMOTE_PARTLY: u64 = 20;
/// `vor_ort`: the largest remote share that still means presence on site.
pub(crate) const ONSITE_MAX: u64 = 60;
/// A remote share at most this many points short of the wish is near, not missed.
pub(crate) const REMOTE_MARGIN: u64 = 30;
/// Remote share (percent, from-to) of hybrid wording without numbers, and of remote work
/// named without numbers and without presence on site.
pub(crate) const HYBRID_SHARE: (u64, u64) = (20, 60);
pub(crate) const REMOTE_NAMED_SHARE: (u64, u64) = (60, 100);
/// Workdays of a week (`zwei Tage vor Ort` = 60 % remote).
pub(crate) const WORKDAYS: u64 = 5;
/// A place outside the wished regions with at least this remote share is near, not missed.
pub(crate) const REGION_REMOTE_NEAR: u64 = 60;
