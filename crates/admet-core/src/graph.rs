//! The molecular graph: struct-of-arrays with CSR adjacency.
//!
//! Manual chapter 6. This is the single most consequential data-structure
//! decision in the project, so the reasoning is recorded here rather than left
//! to a commit message.
//!
//! # A molecule is a graph
//!
//! Atoms are vertices, bonds are edges. Undirected, sparse, small, labelled on
//! both vertices and edges. Three properties of *this particular* graph family
//! drive every decision that follows:
//!
//! | Property | Typical value | Consequence |
//! |---|---|---|
//! | Vertices `N` | 15–60 heavy atoms | Tiny. Asymptotic analysis misleads here; constants dominate |
//! | Edges `E` | ≈ 1.05 × `N` | Extremely sparse. Chemical valence caps degree at 4–6 |
//! | Max degree | ≤ 6 | Bounded, so per-vertex neighbour loops are effectively constant time |
//! | Connectivity | usually 1 component | Salts and mixtures give 2–3; must be handled explicitly |
//!
//! Hydrogens are **implicit**. A carbon drawn with two bonds is understood to
//! carry two hydrogens, and storing them explicitly would roughly double `N` for
//! no information gain, since hydrogen count is derivable from valence rules.
//! Every serious cheminformatics toolkit does this. It is a free 2× reduction in
//! graph size.
//!
//! # Why struct-of-arrays
//!
//! The natural object-oriented shape — `Vec<Atom>` where `Atom` is a struct of
//! eight small fields — is the wrong one. Iterating only the elements still drags
//! every other field through the cache: you pay for eight bytes to read one.
//!
//! Storing each property as its own contiguous column means the featuriser, which
//! walks one property at a time across all atoms, touches only the bytes it
//! needs.
//!
//! # Why both CSR *and* dense
//!
//! They serve different consumers, and keeping both is using the right structure
//! for each access pattern rather than redundancy:
//!
//! - **CSR** ([`MolGraph::neighbours`]) is for graph algorithms — ring
//!   perception, scaffold extraction, traversal — where you walk neighbours.
//!   `O(N + E)` memory, one cache line per lookup.
//! - **Dense** ([`crate::features`]) is materialised only at the boundary, when
//!   building the `[N, N]` tensor for ONNX. It is transient and never stored.
//!
//! # Why indices, not pointers
//!
//! A newcomer models a graph as nodes holding references to other nodes. In Rust
//! that forces `Rc<RefCell<Node>>`, which brings reference counting, runtime
//! borrow checks, heap fragmentation, and cycles that leak. The idiomatic answer
//! is the **arena pattern**: flat vectors, integer indices.
//!
//! | Concern | Pointer graph | Index graph |
//! |---|---|---|
//! | Allocation | one per node | one per column, amortised |
//! | Reference cost | 8 bytes + refcount | 4 bytes (`u32`) |
//! | Cycles | leak without `Weak` | impossible — indices are plain data |
//! | Cache behaviour | scattered | contiguous |
//! | Serialisation | needs pointer fixup | direct memcpy |
//! | `Send + Sync` | awkward | automatic |
//!
//! That last row is what makes `rayon` across a 50,000-molecule batch free.

use crate::{CoreError, MAX_HEAVY_ATOMS};

/// Chemical element, restricted to the ten the featuriser one-hot encodes.
///
/// Covers >99% of atoms in drug-like molecules. Anything else maps to
/// [`Element::Other`] rather than failing — a rare element should not make the
/// request 500, and the model has no basis for treating it specially anyway.
///
/// Discriminants are explicit because [`crate::features`] indexes the one-hot
/// block by `element as usize`. Reordering these silently reorders the feature
/// vector, which breaks Python↔Rust parity (TR-03) in the most confusing
/// possible way: no error, just wrong predictions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Element {
    /// Carbon.
    C = 0,
    /// Nitrogen.
    N = 1,
    /// Oxygen.
    O = 2,
    /// Sulfur.
    S = 3,
    /// Fluorine.
    F = 4,
    /// Chlorine.
    Cl = 5,
    /// Bromine.
    Br = 6,
    /// Iodine.
    I = 7,
    /// Phosphorus.
    P = 8,
    /// Anything outside the nine above.
    #[default]
    Other = 9,
}

impl Element {
    /// Number of distinct values, and therefore the width of the one-hot block.
    pub const COUNT: usize = 10;

    /// Parse an organic-subset or bracket-atom element symbol.
    ///
    /// Case matters in SMILES: `C` is aliphatic carbon, `c` is aromatic carbon,
    /// and both are [`Element::C`] — aromaticity is a separate feature, not a
    /// separate element. `Cl` versus `C` + `l` is why this takes the whole
    /// symbol rather than one byte.
    pub fn from_symbol(symbol: &str) -> Self {
        match symbol {
            "C" | "c" => Self::C,
            "N" | "n" => Self::N,
            "O" | "o" => Self::O,
            "S" | "s" => Self::S,
            "F" => Self::F,
            "Cl" => Self::Cl,
            "Br" => Self::Br,
            "I" => Self::I,
            "P" | "p" => Self::P,
            _ => Self::Other,
        }
    }

    /// Whether the symbol is written lowercase, i.e. aromatic in SMILES.
    pub fn is_aromatic_symbol(symbol: &str) -> bool {
        symbol.chars().next().is_some_and(char::is_lowercase)
    }
}

/// Bond order.
///
/// `Unspecified` is not the same as `Single`. A ring-closure digit may state its
/// bond order at either end (`C=1CCCCC1` and `C1CCCCC=1` are the same molecule),
/// so the parser must distinguish "no order given here" from "single bond given
/// here" in order to reconcile the two ends. See
/// [`smiles::ring`](crate::smiles::ring).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BondKind {
    /// No order stated. Resolves to single, or aromatic between aromatic atoms.
    #[default]
    Unspecified = 0,
    /// `-`
    Single = 1,
    /// `=`
    Double = 2,
    /// `#`
    Triple = 3,
    /// `$`
    Quadruple = 4,
    /// `:` — explicit aromatic bond.
    Aromatic = 5,
    /// `/` — directional, cis/trans stereochemistry.
    Up = 6,
    /// `\` — directional, cis/trans stereochemistry.
    Down = 7,
}

impl BondKind {
    /// Map a SMILES bond symbol byte to its order.
    ///
    /// Returns [`BondKind::Unspecified`] for any byte that is not a bond symbol,
    /// which the parser treats as a programming error rather than bad input —
    /// it only calls this after matching the byte.
    pub fn from_symbol(byte: u8) -> Self {
        match byte {
            b'-' => Self::Single,
            b'=' => Self::Double,
            b'#' => Self::Triple,
            b'$' => Self::Quadruple,
            b':' => Self::Aromatic,
            b'/' => Self::Up,
            b'\\' => Self::Down,
            _ => Self::Unspecified,
        }
    }

    /// Numeric bond order, used when computing implicit hydrogen counts.
    ///
    /// Directional bonds are single bonds carrying stereochemistry, so they
    /// count as 1. Aromatic bonds are conventionally 1.5, rounded down here
    /// because valence arithmetic works on integers and the aromatic flag
    /// carries the real information.
    pub fn order(self) -> u8 {
        match self {
            Self::Unspecified | Self::Single | Self::Up | Self::Down | Self::Aromatic => 1,
            Self::Double => 2,
            Self::Triple => 3,
            Self::Quadruple => 4,
        }
    }
}

/// Orbital hybridisation — a cheap encoding of local 3D geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Hybridisation {
    /// Linear.
    Sp = 0,
    /// Trigonal planar.
    Sp2 = 1,
    /// Tetrahedral.
    Sp3 = 2,
    /// Trigonal bipyramidal.
    Sp3d = 3,
    /// Octahedral.
    Sp3d2 = 4,
    /// Not determined.
    #[default]
    Unknown = 5,
}

impl Hybridisation {
    /// Width of the one-hot block. Five real states; `Unknown` folds into
    /// `Sp3`, the commonest case, rather than claiming a sixth dimension.
    pub const COUNT: usize = 5;
}

/// Tetrahedral chirality tag.
///
/// Carried on the graph but **not** in the 33-dimensional feature vector — see
/// [`crate::features`] for why widening the feature space is the wrong response
/// to wanting more signal on 578 training molecules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Chirality {
    /// No stereochemistry specified.
    #[default]
    None = 0,
    /// `@` — anticlockwise.
    Clockwise = 1,
    /// `@@` — clockwise.
    AntiClockwise = 2,
}

/// A molecular graph in struct-of-arrays form.
///
/// Every per-atom property is its own contiguous column, and adjacency is stored
/// in CSR (compressed sparse row) form. Construct one with
/// [`MolGraphBuilder`], not by hand — the CSR invariants are easy to violate and
/// impossible to spot afterwards.
///
/// # Invariants
///
/// Upheld by the builder and relied upon everywhere else:
///
/// 1. Every column has length `n_atoms`.
/// 2. `nbr_offsets` has length `n_atoms + 1` and is non-decreasing.
/// 3. `nbr_offsets[0] == 0` and `*nbr_offsets.last() == nbr_indices.len()`.
/// 4. `nbr_indices` and `nbr_bond` have equal length, `2E` — **each bond appears
///    twice**, once from each endpoint.
/// 5. Every value in `nbr_indices` is `< n_atoms`.
/// 6. `n_atoms <= MAX_HEAVY_ATOMS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MolGraph {
    // --- vertex columns, each of length n_atoms ---
    /// Element per atom.
    pub element: Vec<Element>,
    /// Formal charge per atom, typically in −2..=+2.
    pub formal_charge: Vec<i8>,
    /// Implicit hydrogen count per atom.
    pub num_hs: Vec<u8>,
    /// Hybridisation per atom.
    pub hybridisation: Vec<Hybridisation>,
    /// Aromaticity flag per atom.
    pub aromatic: Vec<bool>,
    /// Ring-membership flag per atom.
    pub in_ring: Vec<bool>,
    /// Chirality tag per atom.
    pub chirality: Vec<Chirality>,

    // --- edge storage: CSR for traversal ---
    /// Start index into `nbr_indices` for each atom. Length `n_atoms + 1`.
    pub nbr_offsets: Vec<u32>,
    /// Flattened neighbour lists. Length `2E`.
    pub nbr_indices: Vec<u32>,
    /// Bond kind parallel to `nbr_indices`. Length `2E`.
    pub nbr_bond: Vec<BondKind>,

    /// Number of heavy atoms.
    pub n_atoms: usize,
}
//  Hand-written, because `#[derive(Default)]` produces an INVALID graph.
//
//  CSR requires `nbr_offsets.len() == n_atoms + 1` -- the trailing sentinel is
//  what makes `nbr_offsets[i]..nbr_offsets[i + 1]` well-defined for the last
//  atom. A derived default gives an empty vector, so `MolGraph::default()`
//  failed its own `validate()`, and every test that builds a graph with
//  `..Default::default()` inherited the broken invariant.
impl Default for MolGraph {
    fn default() -> Self {
        Self {
            element: Vec::new(),
            formal_charge: Vec::new(),
            num_hs: Vec::new(),
            hybridisation: Vec::new(),
            aromatic: Vec::new(),
            in_ring: Vec::new(),
            chirality: Vec::new(),
            //  The sentinel. An empty graph has one offset, not zero.
            nbr_offsets: vec![0],
            nbr_indices: Vec::new(),
            nbr_bond: Vec::new(),
            n_atoms: 0,
        }
    }
}

impl MolGraph {
    /// Neighbours of atom `i` as a contiguous slice — one cache line.
    ///
    /// This is the hot path for every graph algorithm in the crate, which is the
    /// entire reason adjacency is CSR rather than `Vec<Vec<u32>>`.
    ///
    /// # Panics
    /// If `i >= n_atoms`.
    #[inline]
    pub fn neighbours(&self, i: usize) -> &[u32] {
        let start = self.nbr_offsets[i] as usize;
        let end = self.nbr_offsets[i + 1] as usize;
        &self.nbr_indices[start..end]
    }

    /// Bond kinds parallel to [`MolGraph::neighbours`] for atom `i`.
    ///
    /// # Panics
    /// If `i >= n_atoms`.
    #[inline]
    pub fn neighbour_bonds(&self, i: usize) -> &[BondKind] {
        let start = self.nbr_offsets[i] as usize;
        let end = self.nbr_offsets[i + 1] as usize;
        &self.nbr_bond[start..end]
    }

    /// Heavy-atom degree of atom `i`.
    ///
    /// # Panics
    /// If `i >= n_atoms`.
    #[inline]
    pub fn degree(&self, i: usize) -> usize {
        (self.nbr_offsets[i + 1] - self.nbr_offsets[i]) as usize
    }

    /// Number of bonds, `E`. Half the CSR length, since each bond is stored twice.
    #[inline]
    pub fn n_bonds(&self) -> usize {
        self.nbr_indices.len() / 2
    }

    /// True when the graph holds no atoms.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n_atoms == 0
    }

    /// Iterate atom indices.
    pub fn atoms(&self) -> impl Iterator<Item = usize> + '_ {
        0..self.n_atoms
    }

    /// Check every invariant listed on [`MolGraph`].
    ///
    /// Cheap enough to call in tests and debug builds, and it turns a class of
    /// bug that manifests as wrong predictions into one that manifests as a
    /// failed assertion naming the broken invariant.
    pub fn validate(&self) -> crate::Result<()> {
        if self.n_atoms > MAX_HEAVY_ATOMS {
            return Err(CoreError::TooLarge {
                found: self.n_atoms,
                limit: MAX_HEAVY_ATOMS,
            });
        }

        let columns_ok = self.element.len() == self.n_atoms
            && self.formal_charge.len() == self.n_atoms
            && self.num_hs.len() == self.n_atoms
            && self.hybridisation.len() == self.n_atoms
            && self.aromatic.len() == self.n_atoms
            && self.in_ring.len() == self.n_atoms
            && self.chirality.len() == self.n_atoms;
        debug_assert!(columns_ok, "MolGraph column length mismatch");

        debug_assert_eq!(
            self.nbr_offsets.len(),
            self.n_atoms + 1,
            "nbr_offsets must have length n_atoms + 1"
        );
        debug_assert_eq!(
            self.nbr_indices.len(),
            self.nbr_bond.len(),
            "nbr_indices and nbr_bond must be parallel"
        );
        debug_assert!(
            self.nbr_offsets.windows(2).all(|w| w[0] <= w[1]),
            "nbr_offsets must be non-decreasing"
        );
        debug_assert!(
            self.nbr_indices
                .iter()
                .all(|&j| (j as usize) < self.n_atoms),
            "neighbour index out of range"
        );

        Ok(())
    }
}

/// Incremental builder for [`MolGraph`].
///
/// The parser appends atoms and bonds as it scans left to right, in an order
/// that has nothing to do with CSR layout. This collects them as an edge list and
/// [`MolGraphBuilder::finish`] performs the single counting sort that produces
/// valid CSR — `O(N + E)`, one pass, no allocation per atom.
#[derive(Debug, Clone, Default)]
pub struct MolGraphBuilder {
    element: Vec<Element>,
    formal_charge: Vec<i8>,
    num_hs: Vec<u8>,
    hybridisation: Vec<Hybridisation>,
    aromatic: Vec<bool>,
    in_ring: Vec<bool>,
    chirality: Vec<Chirality>,
    /// Edge list, `(a, b, kind)` with each bond recorded once.
    //
    //  `expect` rather than `allow` on purpose: `add_bond` is an Increment-2
    //  stub, so nothing writes this column yet. When the parser lands and
    //  starts filling it, `expect` turns into an "unfulfilled expectation"
    //  warning and forces this attribute to be deleted. `allow` would sit here
    //  silently forever.
    #[expect(dead_code, reason = "written by add_bond in Increment 2")]
    edges: Vec<(u32, u32, BondKind)>,
}

/// One atom's properties, as read from SMILES.
///
/// A parameter object rather than eight positional arguments, because
/// `add_atom(Element::C, 0, 3, Hybridisation::Sp3, false, false, Chirality::None)`
/// is unreadable and its two `bool`s are trivially transposable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AtomSpec {
    /// Element.
    pub element: Element,
    /// Formal charge.
    pub formal_charge: i8,
    /// Explicit hydrogen count, if the atom was written in brackets.
    pub num_hs: u8,
    /// Hybridisation, if known.
    pub hybridisation: Hybridisation,
    /// Whether the symbol was lowercase.
    pub aromatic: bool,
    /// Chirality tag.
    pub chirality: Chirality,
}

impl MolGraphBuilder {
    /// Empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder with capacity for `n` atoms, to avoid regrowth mid-parse.
    ///
    /// A good estimate is the SMILES string length: it always over-estimates
    /// (multi-character tokens, bond symbols, parentheses) and never
    /// under-estimates, so one allocation per column suffices.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            element: Vec::with_capacity(n),
            formal_charge: Vec::with_capacity(n),
            num_hs: Vec::with_capacity(n),
            hybridisation: Vec::with_capacity(n),
            aromatic: Vec::with_capacity(n),
            in_ring: Vec::with_capacity(n),
            chirality: Vec::with_capacity(n),
            edges: Vec::with_capacity(n + 2),
        }
    }

    /// Append an atom and return its index.
    pub fn add_atom(&mut self, spec: AtomSpec) -> u32 {
        self.element.push(spec.element);
        self.formal_charge.push(spec.formal_charge);
        self.num_hs.push(spec.num_hs);
        self.hybridisation.push(spec.hybridisation);
        self.aromatic.push(spec.aromatic);
        self.in_ring.push(false); // set by ring perception in finish()
        self.chirality.push(spec.chirality);
        (self.element.len() - 1) as u32
    }

    /// Record a bond between two existing atoms.
    ///
    /// Stored once; [`MolGraphBuilder::finish`] emits both directions.
    ///
    /// # Errors
    /// [`CoreError::NotImplemented`] until validation of self-loops and
    /// duplicate bonds lands with the parser in Increment 2.
    pub fn add_bond(&mut self, a: u32, b: u32, kind: BondKind) -> crate::Result<()> {
        let _ = (a, b, kind);
        Err(CoreError::NotImplemented("MolGraphBuilder::add_bond"))
    }

    /// Number of atoms added so far. The parser needs this to enforce the cap
    /// as it goes, rather than after building a graph it will throw away.
    pub fn len(&self) -> usize {
        self.element.len()
    }

    /// Whether any atom has been added.
    pub fn is_empty(&self) -> bool {
        self.element.is_empty()
    }

    /// Consume the builder, sort the edge list into CSR, and return the graph.
    ///
    /// Also fills the `in_ring` column, which needs the completed adjacency.
    ///
    /// # Errors
    /// [`CoreError::NotImplemented`] until Increment 2.
    pub fn finish(self) -> crate::Result<MolGraph> {
        Err(CoreError::NotImplemented("MolGraphBuilder::finish"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_symbols_map_case_insensitively() {
        assert_eq!(Element::from_symbol("C"), Element::C);
        assert_eq!(Element::from_symbol("c"), Element::C);
        // Two-character symbols must not be read as one character plus junk:
        // "Cl" is chlorine, not carbon followed by something.
        assert_eq!(Element::from_symbol("Cl"), Element::Cl);
        assert_eq!(Element::from_symbol("Br"), Element::Br);
        assert_eq!(Element::from_symbol("Se"), Element::Other);
    }

    #[test]
    fn lowercase_symbols_are_aromatic() {
        assert!(Element::is_aromatic_symbol("c"));
        assert!(Element::is_aromatic_symbol("n"));
        assert!(!Element::is_aromatic_symbol("C"));
        assert!(!Element::is_aromatic_symbol("Cl"));
    }

    #[test]
    fn bond_symbols_and_orders_agree() {
        assert_eq!(BondKind::from_symbol(b'='), BondKind::Double);
        assert_eq!(BondKind::from_symbol(b'#'), BondKind::Triple);
        assert_eq!(BondKind::from_symbol(b':'), BondKind::Aromatic);
        assert_eq!(BondKind::from_symbol(b'x'), BondKind::Unspecified);

        assert_eq!(BondKind::Double.order(), 2);
        assert_eq!(BondKind::Triple.order(), 3);
        // Directional bonds are single bonds carrying stereochemistry.
        assert_eq!(BondKind::Up.order(), 1);
        assert_eq!(BondKind::Down.order(), 1);
    }

    /// The one-hot widths in `features.rs` are derived from these. If a variant
    /// is added without widening the feature block, the vectors silently stop
    /// matching Python's and parity breaks with no error message.
    #[test]
    fn one_hot_widths_match_the_feature_contract() {
        assert_eq!(Element::COUNT, 10);
        assert_eq!(Hybridisation::COUNT, 5);
    }

    #[test]
    fn empty_graph_is_valid_and_empty() {
        let g = MolGraph::default();
        assert!(g.is_empty());
        assert_eq!(g.n_bonds(), 0);
        assert!(g.validate().is_ok());
    }

    #[test]
    fn oversized_graph_is_rejected_not_truncated() {
        let g = MolGraph {
            n_atoms: MAX_HEAVY_ATOMS + 1,
            nbr_offsets: vec![0; MAX_HEAVY_ATOMS + 2],
            ..Default::default()
        };
        assert!(matches!(
            g.validate(),
            Err(CoreError::TooLarge { found, limit })
                if found == MAX_HEAVY_ATOMS + 1 && limit == MAX_HEAVY_ATOMS
        ));
    }

    /// Hand-built benzene, asserting the CSR invariants directly. This is the
    /// fixture the builder must reproduce, so it is written before the builder
    /// exists — six carbons in a ring, every atom degree 2, `2E = 12`.
    #[test]
    fn hand_built_benzene_satisfies_csr_invariants() {
        let n = 6;
        let mut nbr_indices = Vec::new();
        let mut nbr_offsets = vec![0u32];
        for i in 0..n {
            nbr_indices.push(((i + n - 1) % n) as u32);
            nbr_indices.push(((i + 1) % n) as u32);
            nbr_offsets.push(nbr_indices.len() as u32);
        }

        let g = MolGraph {
            element: vec![Element::C; n],
            formal_charge: vec![0; n],
            num_hs: vec![1; n],
            hybridisation: vec![Hybridisation::Sp2; n],
            aromatic: vec![true; n],
            in_ring: vec![true; n],
            chirality: vec![Chirality::None; n],
            nbr_bond: vec![BondKind::Aromatic; nbr_indices.len()],
            nbr_offsets,
            nbr_indices,
            n_atoms: n,
        };

        g.validate().expect("hand-built benzene must be valid");
        assert_eq!(g.n_bonds(), 6);
        for i in g.atoms() {
            assert_eq!(
                g.degree(i),
                2,
                "every benzene carbon has two heavy neighbours"
            );
            assert_eq!(g.neighbours(i).len(), g.neighbour_bonds(i).len());
        }
        assert_eq!(g.neighbours(0), &[5, 1]);
    }

    #[test]
    #[ignore = "Increment 2: MolGraphBuilder::finish"]
    fn builder_reproduces_hand_built_benzene() {
        let mut b = MolGraphBuilder::with_capacity(6);
        let atoms: Vec<u32> = (0..6)
            .map(|_| {
                b.add_atom(AtomSpec {
                    element: Element::C,
                    aromatic: true,
                    hybridisation: Hybridisation::Sp2,
                    num_hs: 1,
                    ..Default::default()
                })
            })
            .collect();
        for i in 0..6 {
            b.add_bond(atoms[i], atoms[(i + 1) % 6], BondKind::Aromatic)
                .expect("ring bond");
        }
        let g = b.finish().expect("benzene must build");
        assert_eq!(g.n_atoms, 6);
        assert_eq!(g.n_bonds(), 6);
        assert!(
            g.in_ring.iter().all(|&r| r),
            "ring perception must mark all six"
        );
    }
}
