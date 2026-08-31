//! # admet-core
//!
//! Pure domain logic for ADMETriage. No I/O of any kind — see ADR-02 and the
//! comment block in `Cargo.toml` for why that constraint is load-bearing rather
//! than stylistic.
//!
//! ## What lives here
//!
//! | Module | Responsibility | Manual ch. |
//! |---|---|---|
//! | [`graph`] | `MolGraph`: struct-of-arrays + CSR adjacency | 6 |
//! | [`smiles`] | LL(1) recursive-descent parser with byte-offset errors | 7 |
//! | [`canonical`] | Morgan refinement, canonical ranks, InChIKey | 8 |
//! | [`scaffold`] | Union-Find components, ring perception, Bemis–Murcko | 9 |
//! | [`features`] | The 33-dimensional atom feature contract | 10 |
//! | [`fingerprint`] | `[u64; 32]` Morgan bitset, Tanimoto via popcount | 15 |
//! | [`triage`] | Weighted desirability score, bounded-heap top-k | 14 |
//!
//! ## Scaffold status
//!
//! Every module below is a **typed skeleton**: signatures, doc comments, and the
//! reasoning behind each decision are written; the bodies are not. Each unwritten
//! function returns [`CoreError::NotImplemented`] rather than calling `todo!()`,
//! so the crate compiles clean under `clippy -D warnings` and CI is green from
//! commit one. Tests for unwritten behaviour are `#[ignore]`d with the increment
//! that lands them.
//!
//! Implement in this order — it is dependency order, and each step is testable
//! before the next one exists:
//!
//! 1. [`graph`] — the structure everything else operates on
//! 2. [`smiles`] — produces a `MolGraph`
//! 3. [`features`] — consumes one; unlocks the parity fixture (TR-03)
//! 4. [`canonical`] — identity, which unlocks caching (ADR-04)
//! 5. [`scaffold`], [`fingerprint`], [`triage`] — independent of each other
//!
//! ## The constraint that shapes everything
//!
//! Molecules above [`MAX_HEAVY_ATOMS`] are **rejected, never truncated**. A
//! truncated molecule is a different molecule, and returning a confident
//! prediction about a different molecule is worse than returning an error. That
//! cap is baked into the exported ONNX graph, so it is not a policy this crate
//! is free to relax.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod canonical;
pub mod features;
pub mod fingerprint;
pub mod graph;
pub mod scaffold;
pub mod smiles;
pub mod triage;

pub use graph::{BondKind, Element, Hybridisation, MolGraph};
pub use smiles::SmilesError;

/// Maximum heavy atoms per molecule.
///
/// Fixed in the exported ONNX graph, and mirrored by
/// `admet_infer::MAX_ATOMS` and `MAX_HEAVY_ATOMS` in the Python featuriser.
/// All three must agree; the parity fixture is what proves they do.
///
/// Roughly 99.4% of drug-like compounds fall under this cap. What it excludes is
/// peptides and biologics, which are explicitly out of scope — the platform
/// targets small molecules.
pub const MAX_HEAVY_ATOMS: usize = 128;

/// Atom feature vector width. See [`features`] for the layout.
pub const N_ATOM_FEATURES: usize = 33;

/// Number of ADMET endpoints predicted per molecule.
pub const N_ENDPOINTS: usize = 12;

/// Longest SMILES string accepted, checked before parsing.
///
/// Cheapest possible rejection of garbage input: a length comparison, before any
/// allocation or parsing happens. Defence in depth for the denial-of-service row
/// of the STRIDE table, and requirement FR-03.
pub const MAX_SMILES_LEN: usize = 1_000;

/// Errors from domain operations that are not parse errors.
///
/// Parse failures have their own richer type, [`SmilesError`], because they
/// carry a byte offset used to render a caret under the offending character.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// The molecule exceeds [`MAX_HEAVY_ATOMS`].
    #[error("molecule has {found} heavy atoms, limit is {limit}")]
    TooLarge {
        /// Heavy-atom count of the offending molecule.
        found: usize,
        /// The cap, always [`MAX_HEAVY_ATOMS`].
        limit: usize,
    },

    /// Input was empty or whitespace only.
    #[error("empty input")]
    EmptyInput,

    /// Input exceeded [`MAX_SMILES_LEN`] before parsing was attempted.
    #[error("input is {found} bytes, limit is {limit}")]
    InputTooLong {
        /// Length of the offending input.
        found: usize,
        /// The cap, always [`MAX_SMILES_LEN`].
        limit: usize,
    },

    /// Input contained non-ASCII or control characters.
    #[error("input contains characters that cannot appear in SMILES")]
    IllegalCharacters,

    /// Placeholder for scaffolded behaviour. **Delete this variant once every
    /// module is implemented** — a release build should not be able to construct
    /// it, and leaving it in place lets an unimplemented path reach production
    /// looking like an ordinary error.
    #[error("not implemented yet: {0} (see the module docs for the increment that lands it)")]
    NotImplemented(&'static str),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Cheap validation of raw input, before any parsing is attempted.
///
/// Ordering matters and is deliberate: **length before content, content before
/// parsing, parsing before inference.** An attacker sending garbage is rejected
/// in nanoseconds rather than milliseconds. Manual Listing 25.2.
///
/// ```
/// use admet_core::validate_input;
/// assert!(validate_input("CC(=O)Oc1ccccc1C(=O)O").is_ok());
/// assert!(validate_input("").is_err());
/// assert!(validate_input("CC\u{0}O").is_err());
/// ```
pub fn validate_input(s: &str) -> Result<()> {
    if s.trim().is_empty() {
        return Err(CoreError::EmptyInput);
    }
    if s.len() > MAX_SMILES_LEN {
        return Err(CoreError::InputTooLong {
            found: s.len(),
            limit: MAX_SMILES_LEN,
        });
    }
    if !s.is_ascii() || s.chars().any(char::is_control) {
        return Err(CoreError::IllegalCharacters);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tripwire, not a tautology. If someone edits these constants to "support
    /// bigger molecules" without re-exporting the model, the numbers here and
    /// the ones baked into the ONNX graph diverge, and every prediction silently
    /// becomes garbage. The real defence is the parity test in admet-infer;
    /// this documents the intent and fails faster.
    #[test]
    fn contract_constants_are_stable() {
        assert_eq!(MAX_HEAVY_ATOMS, 128);
        assert_eq!(N_ATOM_FEATURES, 33);
        assert_eq!(N_ENDPOINTS, 12);
    }

    #[test]
    fn validation_rejects_in_cost_order() {
        assert_eq!(validate_input(""), Err(CoreError::EmptyInput));
        assert_eq!(validate_input("   "), Err(CoreError::EmptyInput));

        let long = "C".repeat(MAX_SMILES_LEN + 1);
        assert!(matches!(
            validate_input(&long),
            Err(CoreError::InputTooLong { .. })
        ));

        assert_eq!(
            validate_input("CC\u{00e9}O"),
            Err(CoreError::IllegalCharacters)
        );
        assert_eq!(validate_input("CC\tO"), Err(CoreError::IllegalCharacters));
    }

    #[test]
    fn validation_accepts_real_drugs() {
        for smiles in [
            "CC(=O)Oc1ccccc1C(=O)O",       // aspirin
            "Cn1cnc2c1c(=O)[nH]c(=O)n2C",  // caffeine
            "CC(C)Cc1ccc(cc1)C(C)C(=O)O",  // ibuprofen
            "CC(C)NCC(O)COc1cccc2ccccc12", // propranolol
        ] {
            assert!(validate_input(smiles).is_ok(), "{smiles} should validate");
        }
    }
}
