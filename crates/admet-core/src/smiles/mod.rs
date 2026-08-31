//! SMILES parsing: recursive descent with an explicit stack.
//!
//! Manual chapter 7.
//!
//! # What you are parsing
//!
//! SMILES is a linear string encoding of a molecular graph.
//! `CC(=O)Oc1ccccc1C(=O)O` is aspirin. The notation is compact and
//! human-writable, which is why every chemical database uses it — and it has
//! four features that make parsing non-trivial:
//!
//! | Feature | Example | Parsing implication |
//! |---|---|---|
//! | Chain | `CCCC` | Trivial — bond each atom to the previous one |
//! | Branch | `CC(=O)O` | Parentheses nest. Needs a stack of anchor atoms |
//! | Ring closure | `c1ccccc1` | A digit opens a bond that closes later. Needs a pending-bond table |
//! | Bracket atom | `[nH]`, `[Fe+2]` | A sub-grammar: isotope, element, chirality, H count, charge |
//!
//! # Complexity
//!
//! | Aspect | Cost | Note |
//! |---|---|---|
//! | Time | `O(L)` | `L` = string length. Single pass, no backtracking |
//! | Space — graph | `O(N + E)` | Output size |
//! | Space — branch stack | `O(d)` | `d` = nesting depth, realistically < 10 |
//! | Space — ring table | `O(1)` | Fixed 100 slots |
//!
//! Linear time with no backtracking is achievable because **SMILES is an LL(1)
//! grammar** — one character of lookahead always determines the production. That
//! sentence is precise, correct, and exactly the kind of thing to say in a viva.
//!
//! # Module layout
//!
//! - [`lexer`] — byte-level tokenisation, including the bracket-atom sub-grammar
//! - [`parser`] — the main loop, branch stack, and graph construction
//! - [`ring`] — the ring-closure table, which is the one genuinely clever part

pub mod lexer;
pub mod parser;
pub mod ring;

pub use parser::{parse, Parser};

use crate::graph::BondKind;

/// A parse failure, always carrying enough position information to render a
/// caret under the offending character.
///
/// # Why every variant has a position
///
/// Users are chemists typing structures by hand. "Invalid SMILES" is a useless
/// error message. Carrying the byte offset lets the API render:
///
/// ```text
///   CC(=O)Oc1ccccc2C(=O)O
///                 ^ ring bond 2 was never closed
/// ```
///
/// That is the difference between a tool a chemist tolerates and one they trust.
/// It satisfies **FR-02** (validation feedback carrying a byte offset) and
/// **NFR-06** (malformed input degrades to a typed error, never a panic), and it
/// is a strong live-demo moment: type a broken SMILES on purpose and show the
/// error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SmilesError {
    /// A `(` appeared with no preceding atom to branch from.
    #[error("branch '(' at position {0} has no atom to attach to")]
    BranchAtStart(usize),

    /// A `)` appeared with no matching `(`.
    #[error("unbalanced ')' at position {0}")]
    UnbalancedParen(usize),

    /// The string ended with an open `(`.
    #[error("unclosed branch: {count} '(' never closed")]
    UnclosedBranch {
        /// How many branches remained open.
        count: usize,
    },

    /// A ring-closure digit appeared with no preceding atom.
    #[error("ring closure digit at position {0} has no atom to attach to")]
    RingAtStart(usize),

    /// A ring digit was opened but never closed.
    #[error("ring bond {0} was never closed")]
    UnclosedRing(u8),

    /// The same atom opened and closed one ring digit.
    #[error("ring bond at position {0} would bond an atom to itself")]
    SelfLoop(usize),

    /// A ring bond's order was stated at both ends, and the two disagree.
    ///
    /// Worth its own variant rather than folding into a generic error: it is the
    /// one ring failure a chemist can act on without reading the spec.
    #[error("ring bond {digit} was opened as {first:?} but closed as {second:?}")]
    RingBondMismatch {
        /// The ring digit.
        digit: u8,
        /// Order stated where the ring opened.
        first: BondKind,
        /// Order stated where the ring closed.
        second: BondKind,
    },

    /// An element symbol that is not in the organic subset and not a known
    /// bracket element.
    #[error("unknown element '{symbol}' at position {position}")]
    UnknownElement {
        /// The offending symbol.
        symbol: String,
        /// Byte offset where it starts.
        position: usize,
    },

    /// A `[` with no matching `]`.
    #[error("unclosed bracket atom starting at position {0}")]
    UnclosedBracket(usize),

    /// Malformed content inside `[...]`.
    #[error("malformed bracket atom at position {position}: {reason}")]
    MalformedBracket {
        /// Byte offset of the `[`.
        position: usize,
        /// What specifically was wrong.
        reason: &'static str,
    },

    /// A byte that cannot appear in SMILES.
    #[error("unexpected character {character:?} at position {position}")]
    UnexpectedCharacter {
        /// The offending byte, rendered as a char.
        character: char,
        /// Byte offset.
        position: usize,
    },

    /// The molecule parsed but exceeds the heavy-atom cap.
    ///
    /// Distinct from [`crate::CoreError::TooLarge`] because the parser detects it
    /// mid-scan and can abort early rather than building a graph it must discard.
    #[error("molecule has {found} heavy atoms, limit is {limit}")]
    TooLarge {
        /// Heavy-atom count reached before aborting.
        found: usize,
        /// The cap.
        limit: usize,
    },

    /// Placeholder while the parser is scaffolded. Delete with the last stub.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

impl SmilesError {
    /// Byte offset the error refers to, when it has one.
    ///
    /// `UnclosedRing` and `UnclosedBranch` are detected at end-of-input, so the
    /// meaningful position is where the construct *opened* — which the parser
    /// does not retain. Returning `None` is honest; pointing a caret at the last
    /// character would be worse than pointing at nothing.
    pub fn position(&self) -> Option<usize> {
        match self {
            Self::BranchAtStart(p)
            | Self::UnbalancedParen(p)
            | Self::RingAtStart(p)
            | Self::SelfLoop(p)
            | Self::UnclosedBracket(p) => Some(*p),
            Self::UnknownElement { position, .. }
            | Self::MalformedBracket { position, .. }
            | Self::UnexpectedCharacter { position, .. } => Some(*position),
            Self::UnclosedBranch { .. }
            | Self::UnclosedRing(_)
            | Self::RingBondMismatch { .. }
            | Self::TooLarge { .. }
            | Self::NotImplemented(_) => None,
        }
    }

    /// Render the input with a caret under the offending character.
    ///
    /// The two-line form the API returns in its RFC 9457 `detail` field, and what
    /// the web interface shows beneath the input box.
    ///
    /// ```
    /// use admet_core::smiles::SmilesError;
    /// let err = SmilesError::UnbalancedParen(4);
    /// let rendered = err.render("CCC)C");
    /// assert_eq!(rendered.lines().next().unwrap(), "CCC)C");
    /// assert!(rendered.lines().nth(1).unwrap().starts_with("    ^"));
    /// ```
    pub fn render(&self, input: &str) -> String {
        match self.position() {
            // Char count, not byte count: validate_input() rejects non-ASCII
            // before parsing, so the two agree in practice -- but if that check
            // ever moves, a byte offset would put the caret in the wrong place.
            Some(pos) => {
                let caret_col = input.get(..pos).map_or(pos, |s| s.chars().count());
                format!("{input}\n{}^ {self}", " ".repeat(caret_col))
            }
            None => format!("{input}\n{self}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positioned_errors_expose_their_offset() {
        assert_eq!(SmilesError::UnbalancedParen(7).position(), Some(7));
        assert_eq!(
            SmilesError::UnknownElement {
                symbol: "Xx".into(),
                position: 3
            }
            .position(),
            Some(3)
        );
        // End-of-input errors have no honest position.
        assert_eq!(SmilesError::UnclosedRing(2).position(), None);
        assert_eq!(SmilesError::UnclosedBranch { count: 1 }.position(), None);
    }

    #[test]
    fn caret_lands_under_the_offending_character() {
        let input = "CC(=O)Oc1ccccc2C(=O)O";
        let rendered = SmilesError::SelfLoop(14).render(input);
        let mut lines = rendered.lines();
        assert_eq!(lines.next().unwrap(), input);
        let caret_line = lines.next().unwrap();
        assert_eq!(caret_line.find('^'), Some(14));
    }

    #[test]
    fn unpositioned_errors_render_without_a_caret() {
        let rendered = SmilesError::UnclosedRing(2).render("c1ccccc2");
        assert!(!rendered.contains('^'));
        assert!(rendered.contains("ring bond 2 was never closed"));
    }

    /// The message a chemist reads must name both ends of the disagreement.
    /// "Invalid ring bond" would be true and useless.
    #[test]
    fn ring_mismatch_names_both_orders() {
        let err = SmilesError::RingBondMismatch {
            digit: 1,
            first: BondKind::Double,
            second: BondKind::Single,
        };
        let msg = err.to_string();
        assert!(msg.contains("Double"), "{msg}");
        assert!(msg.contains("Single"), "{msg}");
    }
}
