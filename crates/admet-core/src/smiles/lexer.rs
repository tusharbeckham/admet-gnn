//! Byte-level tokenisation, including the bracket-atom sub-grammar.
//!
//! Manual chapter 7.1. Split from [`super::parser`] because the bracket grammar
//! (`[13CH4+]`: isotope, element, chirality, hydrogen count, charge, class) is a
//! self-contained problem with its own edge cases, and mixing it into the main
//! loop makes both harder to read and to test.
//!
//! Everything here operates on `&[u8]`, not `&str`. SMILES is ASCII by
//! definition, [`crate::validate_input`] enforces that before parsing, and byte
//! indexing avoids UTF-8 boundary checks in the hottest loop in the crate.

use crate::graph::{AtomSpec, BondKind};
use crate::smiles::SmilesError;

/// Elements writable without brackets: the "organic subset".
///
/// Everything else needs `[...]`. Two-character symbols must be tried before
/// one-character ones, or `Cl` lexes as carbon followed by a stray `l`.
pub const ORGANIC_SUBSET: &[&str] = &[
    // two characters first -- order matters
    "Cl", "Br", // then one
    "B", "C", "N", "O", "P", "S", "F", "I", // aromatic forms
    "b", "c", "n", "o", "p", "s",
];

/// A single lexical unit of a SMILES string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// An atom, from the organic subset or a bracket expression.
    Atom(AtomSpec),
    /// An explicit bond symbol.
    Bond(BondKind),
    /// `(` — open a branch.
    BranchOpen,
    /// `)` — close a branch.
    BranchClose,
    /// A ring-closure digit, `0`–`9` or `%nn` for 10 and above.
    RingClosure(u8),
    /// `.` — component separator (salts, mixtures).
    Dot,
}

/// Cursor over the input bytes.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Wrap an input string.
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    /// Current byte offset. Every [`SmilesError`] position comes from here.
    #[inline]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Next byte without consuming it. The single character of lookahead that
    /// makes the grammar LL(1).
    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    /// Byte after next, without consuming. Needed only to distinguish `C` from
    /// `Cl`/`Cr` and `@` from `@@`.
    #[inline]
    pub fn peek2(&self) -> Option<u8> {
        self.input.get(self.pos + 1).copied()
    }

    /// Advance one byte.
    #[inline]
    pub fn bump(&mut self) {
        self.pos += 1;
    }

    /// Whether the input is exhausted.
    #[inline]
    pub fn is_done(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// Consume an organic-subset atom symbol.
    ///
    /// # Errors
    /// [`SmilesError::NotImplemented`] until Increment 2.
    pub fn organic_atom(&mut self) -> Result<AtomSpec, SmilesError> {
        Err(SmilesError::NotImplemented("Lexer::organic_atom"))
    }

    /// Consume a bracket atom, `[` through `]`.
    ///
    /// Grammar, in order, every part optional except the symbol:
    /// `[` isotope? symbol chirality? hcount? charge? class? `]`
    ///
    /// Note that a bracket atom's hydrogen count is **explicit** — `[nH]` has
    /// exactly one, and unlike the organic subset there is no valence inference.
    /// Getting that wrong makes every N-heterocycle's feature vector subtly
    /// disagree with RDKit's, which is a parity failure (TR-03) with no obvious
    /// cause.
    ///
    /// # Errors
    /// [`SmilesError::NotImplemented`] until Increment 2.
    pub fn bracket_atom(&mut self) -> Result<AtomSpec, SmilesError> {
        Err(SmilesError::NotImplemented("Lexer::bracket_atom"))
    }

    /// Consume a single ring-closure digit.
    ///
    /// # Errors
    /// [`SmilesError::NotImplemented`] until Increment 2.
    pub fn ring_digit(&mut self) -> Result<u8, SmilesError> {
        Err(SmilesError::NotImplemented("Lexer::ring_digit"))
    }

    /// Consume `%nn`, a two-digit ring closure for labels 10–99.
    ///
    /// # Errors
    /// [`SmilesError::NotImplemented`] until Increment 2.
    pub fn ring_two_digit(&mut self) -> Result<u8, SmilesError> {
        Err(SmilesError::NotImplemented("Lexer::ring_two_digit"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_character_symbols_precede_one_character_ones() {
        // If this ordering breaks, "Cl" lexes as carbon + stray 'l' and every
        // chlorinated compound silently gains an atom. Cheap test, nasty bug.
        let cl = ORGANIC_SUBSET.iter().position(|s| *s == "Cl").unwrap();
        let c = ORGANIC_SUBSET.iter().position(|s| *s == "C").unwrap();
        assert!(cl < c, "Cl must be matched before C");

        let br = ORGANIC_SUBSET.iter().position(|s| *s == "Br").unwrap();
        let b = ORGANIC_SUBSET.iter().position(|s| *s == "B").unwrap();
        assert!(br < b, "Br must be matched before B");
    }

    #[test]
    fn cursor_tracks_position_and_lookahead() {
        let mut lx = Lexer::new("CCO");
        assert_eq!(lx.position(), 0);
        assert_eq!(lx.peek(), Some(b'C'));
        assert_eq!(lx.peek2(), Some(b'C'));
        lx.bump();
        assert_eq!(lx.position(), 1);
        lx.bump();
        assert_eq!(lx.peek(), Some(b'O'));
        assert_eq!(lx.peek2(), None);
        lx.bump();
        assert!(lx.is_done());
        assert_eq!(lx.peek(), None);
    }

    #[test]
    #[ignore = "Increment 2: Lexer::organic_atom"]
    fn organic_atoms_carry_aromaticity_from_case() {
        use crate::graph::Element;

        let mut lx = Lexer::new("c");
        let a = lx.organic_atom().unwrap();
        assert_eq!(a.element, Element::C);
        assert!(a.aromatic);

        let mut lx = Lexer::new("Cl");
        let a = lx.organic_atom().unwrap();
        assert_eq!(a.element, Element::Cl);
        assert!(!a.aromatic);
        assert!(lx.is_done(), "both bytes of Cl must be consumed");
    }

    #[test]
    #[ignore = "Increment 2: Lexer::bracket_atom"]
    fn bracket_hydrogen_count_is_explicit() {
        use crate::graph::Element;

        let mut lx = Lexer::new("[nH]");
        let a = lx.bracket_atom().unwrap();
        assert_eq!(a.element, Element::N);
        assert!(a.aromatic);
        assert_eq!(a.num_hs, 1, "[nH] carries exactly one hydrogen");
    }

    #[test]
    #[ignore = "Increment 2: Lexer::bracket_atom"]
    fn bracket_charge_parses_both_notations() {
        // [Fe+2] and [Fe++] both mean +2. Supporting only one is a silent
        // wrong-answer bug on real database input.
        for input in ["[Fe+2]", "[Fe++]"] {
            let mut lx = Lexer::new(input);
            let a = lx.bracket_atom().unwrap();
            assert_eq!(a.formal_charge, 2, "{input}");
        }
        let mut lx = Lexer::new("[O-]");
        assert_eq!(lx.bracket_atom().unwrap().formal_charge, -1);
    }
}
