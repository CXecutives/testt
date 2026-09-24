//! Matching engine: scores a job text against the consultant profile.
//!
//! Pure, synchronous and deterministic; no I/O. [`legacy`] reproduces the old Python
//! engine for parity tests.

mod criteria;
mod ladder;
mod lexicon;
mod normalize;
mod profile;
mod pyre;
mod requirements;
mod score;
mod sections;
mod signals;

#[doc(hidden)]
pub mod legacy;
