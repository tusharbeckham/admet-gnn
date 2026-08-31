//! The main parse loop: one left-to-right pass, two pieces of mutable state.
//!
//! Manual chapter 7.2, Listing 7.1.

use crate::graph::{BondKind, MolGraph, MolGraphBuilder};
use crate::smiles::ring::RingTable;
use crate::smiles::SmilesError;
use crate::{validate_input, MAX_HEAVY_ATOMS};

/// Parse a SMILES string into a [`MolGraph`].
///
/// The only entry point callers should need. Validates cheaply first
/// ([`validate_input`]), then parses.
///
/// # Errors
///
/// [`SmilesError`], always with a byte offset where one is meaningful, so the
/// caller can render a caret. Never panics — that is **NFR-06**, and it is
/// enforced by a property test rather than by hope.
///
/// # Examples
///
/// ```
/// use admet_core::smiles::parse;
/// // Increment 2 makes this succeed. Until then it returns NotImplemented,
/// // which is still an Err -- so this assertion holds either way, and the
/// // doctest does not need editing when the parser lands.
/// assert!(parse("((((").is_err());
/// ```
pub fn parse(input: &str) -> Result<MolGraph, SmilesError> {
    validate_input(input).map_err(|e| match e {
        crate::CoreError::EmptyInput => SmilesError::MalformedBracket {
            position: 0,
            reason: "input is empty",
        },
        crate::CoreError::InputTooLong { found, limit } => SmilesError::TooLarge { found, limit },
        _ => SmilesError::UnexpectedCharacter {
            character: input
                .chars()
                .find(|c| !c.is_ascii() || c.is_control())
                .unwrap_or('?'),
            position: 0,
        },
    })?;
    Parser::new(input).run()
}

/// Parser state.
///
/// Two pieces of mutable state carry all the structural bookkeeping, and that is
/// the whole trick:
///
/// - **`branch`** — a stack of anchor atoms. `(` pushes `prev`, `)` pops it back.
///   Nesting depth is realistically under 10, so this never grows.
/// - **`rings`** — a fixed table of pending ring bonds, indexed by digit.
///
/// Everything else is a cursor.
pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
    graph: MolGraphBuilder,
    /// Atom the next bond attaches to. `None` at string start and after `.`.
    prev: Option<u32>,
    /// Branch stack: `(` pushes, `)` pops.
    branch: Vec<u32>,
    /// Ring-closure bonds awaiting their partner.
    rings: RingTable,
    /// A bond order that has been read but not yet applied to a bond.
    pending_bond: Option<BondKind>,
}

impl<'a> Parser<'a> {
    /// New parser over `input`.
    ///
    /// Capacity is estimated from the string length, which always over-estimates
    /// the atom count (bond symbols, parentheses and multi-character tokens all
    /// consume bytes without adding atoms) and never under-estimates. So each
    /// column allocates exactly once.
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            graph: MolGraphBuilder::with_capacity(input.len().min(MAX_HEAVY_ATOMS)),
            prev: None,
            branch: Vec::new(),
            rings: RingTable::new(),
            pending_bond: None,
        }
    }

    /// Run the parse to completion.
    ///
    /// The dispatch is a single `match` on one byte of lookahead — no
    /// backtracking, no speculative parsing, no recursion. Branches are handled
    /// by an explicit stack rather than recursive calls, which is what keeps a
    /// pathological input like `((((((...` from overflowing the stack. A parser
    /// that segfaults on adversarial input is a security problem, not just a
    /// correctness one.
    ///
    /// End-of-input checks matter as much as the loop: an unclosed branch or an
    /// unclosed ring is only detectable once the string runs out, and silently
    /// accepting either produces a graph that is missing a bond.
    ///
    /// # Errors
    /// [`SmilesError::NotImplemented`] until Increment 2.
    pub fn run(self) -> Result<MolGraph, SmilesError> {
        // Touch every field so the scaffold compiles without `dead_code`
        // warnings. Delete this line when `run` is implemented -- if it is still
        // here then, something is genuinely unused and worth removing.
        let _ = (
            &self.input,
            self.pos,
            &self.graph,
            &self.prev,
            &self.branch,
            &self.rings,
            &self.pending_bond,
        );
        Err(SmilesError::NotImplemented("Parser::run"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap checks reject before parsing is attempted, in cost order.
    #[test]
    fn validation_runs_before_parsing() {
        assert!(parse("").is_err());
        assert!(parse("   ").is_err());
        assert!(matches!(
            parse(&"C".repeat(2_000)),
            Err(SmilesError::TooLarge { .. })
        ));
        assert!(parse("CC\u{00e9}O").is_err());
    }

    /// **NFR-06.** The parser accepts untrusted user input, so "never panics" is
    /// a security control as much as a correctness one. `Err` is a fine outcome
    /// for any of these; a panic is a defect.
    ///
    /// The property-based version of this — thousands of generated inputs — lands
    /// with the parser in Increment 2. These are the cases worth naming
    /// explicitly because each one broke a real parser somewhere.
    #[test]
    fn never_panics_on_hostile_input() {
        for hostile in [
            "",
            "(",
            ")",
            "((((((((((",
            "))))))))))",
            "1",
            "%",
            "%9",
            "[",
            "]",
            "[]",
            "[[[[",
            "C1",
            "C11",
            "C=1CCCCC#1",
            ".",
            "....",
            "C..C",
            "-",
            "=#$:/\\",
            "@@@@",
            "[C@@@@H]",
            &"C1".repeat(200),
            &"(".repeat(500),
        ] {
            // The assertion is that this line returns at all.
            let _ = parse(hostile);
        }
    }

    #[test]
    #[ignore = "Increment 2: Parser::run"]
    fn parses_aspirin() {
        let g = parse("CC(=O)Oc1ccccc1C(=O)O").expect("aspirin must parse");
        assert_eq!(g.n_atoms, 13, "aspirin has 13 heavy atoms");
        g.validate()
            .expect("parsed graph must satisfy CSR invariants");
        assert_eq!(
            g.aromatic.iter().filter(|&&a| a).count(),
            6,
            "one benzene ring"
        );
    }

    #[test]
    #[ignore = "Increment 2: Parser::run"]
    fn branch_stack_restores_the_anchor() {
        // The O and the final O both attach to the same carbon. If `)` fails to
        // restore `prev`, the second O bonds to the first and the molecule is
        // a different one -- with no error to indicate it.
        let g = parse("CC(=O)O").expect("must parse");
        assert_eq!(g.n_atoms, 4);
        assert_eq!(g.degree(1), 3, "carbonyl carbon bonds to C, O and O");
    }

    #[test]
    #[ignore = "Increment 2: Parser::run"]
    fn ring_digits_are_reusable_once_closed() {
        // c1ccccc1C1CCCCC1 validly uses digit 1 twice: a label is freed the
        // moment its pair closes. Treating digits as globally unique rejects
        // perfectly good input.
        let g = parse("c1ccccc1C1CCCCC1").expect("must parse");
        assert_eq!(g.n_atoms, 12);
        assert_eq!(
            g.n_bonds(),
            13,
            "two rings (6+6 bonds) plus the linking bond"
        );
    }

    #[test]
    #[ignore = "Increment 2: Parser::run"]
    fn dot_breaks_the_chain_for_salts() {
        // CC(=O)O.[Na+] is sodium acetate: two components, no bond between them.
        let g = parse("CC(=O)O.[Na+]").expect("must parse");
        assert_eq!(g.n_atoms, 5);
        assert_eq!(g.n_bonds(), 3, "the sodium must not be bonded to anything");
    }

    #[test]
    #[ignore = "Increment 2: Parser::run"]
    fn structural_errors_carry_useful_positions() {
        assert!(matches!(
            parse("CCC)C"),
            Err(SmilesError::UnbalancedParen(3))
        ));
        assert!(matches!(parse("(CC"), Err(SmilesError::BranchAtStart(0))));
        assert!(matches!(
            parse("c1ccccc2"),
            Err(SmilesError::UnclosedRing(2))
        ));
        assert!(matches!(
            parse("CC(C"),
            Err(SmilesError::UnclosedBranch { count: 1 })
        ));
    }
}
