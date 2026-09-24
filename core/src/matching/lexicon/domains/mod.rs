//! Domain packs: bilingual pairs and paraphrases of one field on top of the general core
//! (`lexicon::engine`). A pack is switched on automatically when a token of the profile's
//! competences starts with one of its triggers; it never adds matches for other profiles.
//! A new field is one more file here plus one line in [`DOMAINS`].
//!
//! external contract - do not translate: German and English wording of job ads and
//! consultant profiles. Keys are words separated by single spaces (stemmed when the table
//! is built; stopwords and fillers never occur in keys), values are one concept.

mod finance;
mod it_project;
mod sap;

/// One domain pack.
pub(crate) struct Domain {
    /// Stable id, shown in the profile summary.
    pub name: &'static str,
    /// Folded token prefixes in the profile's competences that switch the pack on.
    pub triggers: &'static [&'static str],
    /// Phrase -> concept.
    pub concepts: &'static [(&'static str, &'static str)],
}

/// All packs, in a fixed order.
pub(crate) const DOMAINS: &[&Domain] = &[&finance::DOMAIN, &sap::DOMAIN, &it_project::DOMAIN];
