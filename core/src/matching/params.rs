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
