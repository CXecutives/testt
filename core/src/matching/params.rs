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
