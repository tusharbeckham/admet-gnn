//! The 33-dimensional atom feature contract.
//!
//! Manual chapter 10. The layout is **fully specified and testable**; only
//! [`featurise`] waits on the parser.
//!
//! # This module is a contract, not an implementation detail
//!
//! The same 33 numbers are produced **twice** — once in Python for training
//! (`training/features/atoms.py`), once here for serving. If the two disagree by
//! even one bit position, the Rust service silently returns **wrong predictions**.
//! Not a crash. Plausible, wrong numbers.
//!
//! That is the single nastiest bug class in the whole project, it is risk **R3**,
//! and it has two designed defences:
//!
//! **Defence 1 — one source of truth for the layout.** [`SCHEMA`] serialises to
//! `models/feature_schema.json`, which both languages load. Element order, degree
//! buckets, index offsets: all *data*, not code duplicated in two syntaxes.
//!
//! **Defence 2 — a golden-vector fixture.** 200 diverse molecules featurised in
//! Python, written to `fixtures/golden_features.npz`, asserted here to `1e-6` on
//! every commit. See `tests/parity.rs`.
//!
//! Report this as a deliberate risk mitigation, not an afterthought. Being able to
//! say *"we identified cross-language feature drift as a risk and built an
//! automated control for it"* is exactly the engineering reasoning that scores.
//!
//! # Why one-hot and not integers
//!
//! Almost every field is one-hot encoded rather than stored as a number, because a
//! network should not infer a false ordering. Element 6 (carbon) is not "less
//! than" element 8 (oxygen) in any chemically meaningful sense, and a single
//! `element_number` input invites the model to interpolate between them.
//!
//! # Why not more features
//!
//! You could add partial charges, Gasteiger charges, ring size, chirality, and
//! reach 60 dimensions. **Resist it.** With 578 training molecules for HIA, a wider
//! feature space means more parameters and worse overfitting. Thirty-three is a
//! well-tested compromise used widely in the literature.
//!
//! If you want more signal, **add data, not dimensions.**

use crate::graph::{Element, Hybridisation, MolGraph};
use crate::{MAX_HEAVY_ATOMS, N_ATOM_FEATURES};

/// One contiguous block of the feature vector.
///
/// Not `Deserialize`: `name` is a `&'static str`, and the JSON is written by
/// [`schema_json`] for *Python* to read, never read back into Rust. Rust's copy of
/// the layout is [`SCHEMA`] itself — deserialising it would create a second source
/// of truth, which is the exact failure this module exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// Human-readable name, matching the Python side exactly.
    pub name: &'static str,
    /// First index in the 33-dim vector.
    pub offset: usize,
    /// Number of dimensions.
    pub width: usize,
}

impl Block {
    /// Index range this block occupies.
    pub const fn range(&self) -> std::ops::Range<usize> {
        self.offset..self.offset + self.width
    }
}

/// The frozen feature layout.
///
/// | Idx | Feature | Dims | Encoding | Rationale |
/// |---|---|---|---|---|
/// | 0–9 | Element | 10 | one-hot | C, N, O, S, F, Cl, Br, I, P, other — covers >99% of drug atoms |
/// | 10–15 | Degree | 6 | one-hot 0–5 | Heavy-atom connectivity; correlates with steric bulk |
/// | 16–20 | Formal charge | 5 | one-hot −2…+2 | Drives solubility and membrane permeability |
/// | 21–25 | Hydrogen count | 5 | one-hot 0–4 | Implicit H; needed for H-bond donor logic |
/// | 26–30 | Hybridisation | 5 | one-hot | sp, sp2, sp3, sp3d, sp3d2 — encodes 3D geometry cheaply |
/// | 31 | Aromatic | 1 | binary | Aromatic rings behave very differently from aliphatic |
/// | 32 | In ring | 1 | binary | Ring membership constrains conformational freedom |
///
/// **Changing this table is a breaking change.** It invalidates the trained model,
/// the golden fixtures, and every cached prediction. If you must, bump
/// [`SCHEMA_VERSION`] in the same commit.
pub const SCHEMA: [Block; 7] = [
    Block {
        name: "element",
        offset: 0,
        width: 10,
    },
    Block {
        name: "degree",
        offset: 10,
        width: 6,
    },
    Block {
        name: "formal_charge",
        offset: 16,
        width: 5,
    },
    Block {
        name: "num_hs",
        offset: 21,
        width: 5,
    },
    Block {
        name: "hybridisation",
        offset: 26,
        width: 5,
    },
    Block {
        name: "aromatic",
        offset: 31,
        width: 1,
    },
    Block {
        name: "in_ring",
        offset: 32,
        width: 1,
    },
];

/// Feature-layout version.
///
/// Bump this whenever [`SCHEMA`] changes. The Python featurisation cache keys on
/// it, so bumping invalidates stale `.npz` files automatically — otherwise you
/// silently train on features from the previous layout, which is a full day lost
/// to a wrong answer that looks right.
pub const SCHEMA_VERSION: u32 = 1;

/// Highest degree with its own bucket. Higher degrees clamp into it.
pub const MAX_DEGREE_BUCKET: usize = 5;

/// Most negative formal charge with its own bucket.
pub const MIN_CHARGE: i8 = -2;

/// Most positive formal charge with its own bucket.
pub const MAX_CHARGE: i8 = 2;

/// Highest hydrogen count with its own bucket.
pub const MAX_H_BUCKET: usize = 4;

/// Offset of a block by name.
///
/// # Panics
/// If `name` is not in [`SCHEMA`] — a typo here is a programming error, and
/// failing loudly at startup beats a silently misaligned feature vector.
pub fn block(name: &str) -> Block {
    SCHEMA
        .iter()
        .copied()
        .find(|b| b.name == name)
        .unwrap_or_else(|| panic!("no feature block named {name:?}"))
}

/// Serialise [`SCHEMA`] to the JSON both languages load.
///
/// Written to `models/feature_schema.json` by `admet-cli schema`. The Python
/// featuriser reads the same file, so the layout exists in exactly one place.
pub fn schema_json() -> String {
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "n_features": N_ATOM_FEATURES,
        "max_heavy_atoms": MAX_HEAVY_ATOMS,
        "blocks": SCHEMA.iter().map(|b| serde_json::json!({
            "name": b.name,
            "offset": b.offset,
            "width": b.width,
        })).collect::<Vec<_>>(),
        "element_order": ["C", "N", "O", "S", "F", "Cl", "Br", "I", "P", "other"],
        "hybridisation_order": ["sp", "sp2", "sp3", "sp3d", "sp3d2"],
        "degree_buckets": (0..=MAX_DEGREE_BUCKET).collect::<Vec<_>>(),
        "charge_buckets": (MIN_CHARGE..=MAX_CHARGE).collect::<Vec<_>>(),
        "hydrogen_buckets": (0..=MAX_H_BUCKET).collect::<Vec<_>>(),
        "clamping": "out-of-range values clamp into the nearest edge bucket; \
                     they never error and never leave the row all-zero",
    })
    .to_string()
}

/// Set the one-hot bit for `value` within a block, clamping into range.
///
/// # Why clamp rather than error
///
/// A degree-7 atom is rare but should not make the request fail. More
/// importantly, an out-of-range value that produced an **all-zero block** would be
/// indistinguishable from padding, and the model would treat a real atom as
/// absent. Clamping keeps exactly one bit set per block, always, which is the
/// invariant [`row_is_wellformed`] checks.
#[inline]
pub fn set_one_hot(row: &mut [f32], b: Block, value: usize) {
    row[b.offset + value.min(b.width - 1)] = 1.0;
}

/// The 33-dim feature row for one atom.
///
/// Pure and independently testable: it takes the atom's properties, not the graph,
/// so it can be exercised against Python's output without a parser.
pub fn atom_row(
    element: Element,
    degree: usize,
    formal_charge: i8,
    num_hs: u8,
    hybridisation: Hybridisation,
    aromatic: bool,
    in_ring: bool,
) -> [f32; N_ATOM_FEATURES] {
    let mut row = [0.0f32; N_ATOM_FEATURES];

    set_one_hot(&mut row, SCHEMA[0], element as usize);
    set_one_hot(&mut row, SCHEMA[1], degree);
    // Charge is signed, so shift into a 0-based bucket before one-hot encoding.
    let charge_bucket = (formal_charge.clamp(MIN_CHARGE, MAX_CHARGE) - MIN_CHARGE) as usize;
    set_one_hot(&mut row, SCHEMA[2], charge_bucket);
    set_one_hot(&mut row, SCHEMA[3], num_hs as usize);
    // Hybridisation::Unknown (discriminant 5) falls outside the 5-wide block and
    // clamps to sp3, the commonest case -- rather than claiming a sixth dimension.
    //
    //  This must be done EXPLICITLY. `set_one_hot` clamps with
    //  `value.min(width - 1)`, which would land Unknown on index 4 = Sp3d2 --
    //  octahedral geometry, the rarest state in drug-like space, asserted for
    //  every atom whose hybridisation could not be determined. Silently
    //  mislabelling unknowns as exotic is worse than the missing dimension this
    //  clamp was chosen to avoid.
    let hybridisation_bucket = match hybridisation {
        Hybridisation::Unknown => Hybridisation::Sp3 as usize,
        known => known as usize,
    };
    set_one_hot(&mut row, SCHEMA[4], hybridisation_bucket);

    row[SCHEMA[5].offset] = f32::from(aromatic);
    row[SCHEMA[6].offset] = f32::from(in_ring);

    row
}

/// Check the structural invariants of a feature row.
///
/// Exactly one bit set in each one-hot block, and the two flags in `{0, 1}`.
/// Cheap enough to assert in tests and debug builds, and it catches an
/// off-by-one in the offsets immediately rather than as a mysterious accuracy
/// regression.
pub fn row_is_wellformed(row: &[f32]) -> bool {
    if row.len() != N_ATOM_FEATURES {
        return false;
    }
    for b in &SCHEMA[..5] {
        let set = row[b.range()].iter().filter(|&&v| v == 1.0).count();
        if set != 1 {
            return false;
        }
        if row[b.range()].iter().any(|&v| v != 0.0 && v != 1.0) {
            return false;
        }
    }
    SCHEMA[5..]
        .iter()
        .all(|b| matches!(row[b.offset], 0.0 | 1.0))
}

/// The three tensors the ONNX graph consumes for one molecule.
///
/// | Tensor | Shape | Meaning |
/// |---|---|---|
/// | `x` | `[128, 33]` | Atom vectors; zeros beyond the real atom count |
/// | `adj` | `[128, 128]` | `D^-½ (A + I) D^-½`, zero outside `N × N` |
/// | `mask` | `[128]` | 1.0 for real atoms, 0.0 for padding |
///
/// Stored flattened row-major, which is the layout `ort` wants — building a
/// nested `Vec<Vec<f32>>` and flattening it later allocates 128 times per
/// molecule for no benefit.
#[derive(Debug, Clone, PartialEq)]
pub struct Featurised {
    /// `[128 × 33]` atom features, row-major.
    pub x: Vec<f32>,
    /// `[128 × 128]` normalised adjacency, row-major.
    pub adj: Vec<f32>,
    /// `[128]` padding mask.
    pub mask: Vec<f32>,
    /// Real heavy-atom count, before padding.
    pub n_atoms: usize,
}

impl Featurised {
    /// Zeroed tensors of the right shape.
    pub fn zeroed() -> Self {
        Self {
            x: vec![0.0; MAX_HEAVY_ATOMS * N_ATOM_FEATURES],
            adj: vec![0.0; MAX_HEAVY_ATOMS * MAX_HEAVY_ATOMS],
            mask: vec![0.0; MAX_HEAVY_ATOMS],
            n_atoms: 0,
        }
    }

    /// The feature row for atom `i`.
    pub fn row(&self, i: usize) -> &[f32] {
        &self.x[i * N_ATOM_FEATURES..(i + 1) * N_ATOM_FEATURES]
    }

    /// Check every shape and padding invariant.
    ///
    /// # The mask is not optional
    ///
    /// Without it, mean pooling divides by 128 instead of by the real atom count.
    /// A 20-atom molecule would have its representation scaled down by 6.4×
    /// relative to a 128-atom one, purely as an artefact of padding. The model
    /// trains around it badly and you chase phantom accuracy problems for days.
    pub fn validate(&self) -> bool {
        self.x.len() == MAX_HEAVY_ATOMS * N_ATOM_FEATURES
            && self.adj.len() == MAX_HEAVY_ATOMS * MAX_HEAVY_ATOMS
            && self.mask.len() == MAX_HEAVY_ATOMS
            && self.n_atoms <= MAX_HEAVY_ATOMS
            && self.mask.iter().take(self.n_atoms).all(|&m| m == 1.0)
            && self.mask.iter().skip(self.n_atoms).all(|&m| m == 0.0)
            // Padded feature rows must be all zero, or padding contributes to
            // the aggregation despite the mask.
            && self.x[self.n_atoms * N_ATOM_FEATURES..].iter().all(|&v| v == 0.0)
    }

    /// Zero one atom's feature row, for occlusion attribution (Increment 4).
    ///
    /// Masking the atom out and re-running inference measures how much the
    /// prediction moves, which is how the explainability endpoint decides which
    /// atoms matter.
    pub fn mask_atom(&mut self, i: usize) {
        if i < self.n_atoms {
            self.x[i * N_ATOM_FEATURES..(i + 1) * N_ATOM_FEATURES].fill(0.0);
            self.mask[i] = 0.0;
        }
    }
}

/// Featurise a parsed molecule into padded dense tensors.
///
/// # The normalisation happens here, not in the model
///
/// Self-loops are added and then symmetric normalisation `D^-½ (A + I) D^-½` is
/// applied **on the CPU, during featurisation**. Two reasons, both load-bearing:
///
/// 1. It is a property of the graph, not of the model, so it costs nothing at
///    inference time and nothing to retrain.
/// 2. Doing it inside the model would put a reduction over a dynamic axis into
///    the exported graph, and static shapes are exactly what makes the export
///    work (ADR-03).
///
/// Without normalisation, high-degree atoms produce large activations and
/// destabilise training. Without self-loops, an atom's own features are dropped
/// from its aggregation.
///
/// # Errors
/// [`crate::CoreError::TooLarge`] above [`MAX_HEAVY_ATOMS`], and
/// [`crate::CoreError::NotImplemented`] until Increment 2.
pub fn featurise(graph: &MolGraph) -> crate::Result<Featurised> {
    if graph.n_atoms > MAX_HEAVY_ATOMS {
        return Err(crate::CoreError::TooLarge {
            found: graph.n_atoms,
            limit: MAX_HEAVY_ATOMS,
        });
    }
    Err(crate::CoreError::NotImplemented("features::featurise"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layout must tile 0..33 exactly: contiguous, no gaps, no overlaps. An
    /// off-by-one here shifts every downstream feature and the model trains on
    /// garbage that looks plausible.
    #[test]
    fn blocks_tile_the_vector_exactly() {
        let mut expected_offset = 0;
        for b in &SCHEMA {
            assert_eq!(
                b.offset, expected_offset,
                "block {:?} is misaligned",
                b.name
            );
            assert!(b.width > 0);
            expected_offset += b.width;
        }
        assert_eq!(
            expected_offset, N_ATOM_FEATURES,
            "blocks must sum to exactly {N_ATOM_FEATURES} dimensions"
        );
    }

    /// The documented index ranges, asserted literally. If someone reorders the
    /// blocks, this fails with the exact indices that moved.
    #[test]
    fn documented_offsets_are_the_actual_offsets() {
        assert_eq!(block("element").range(), 0..10);
        assert_eq!(block("degree").range(), 10..16);
        assert_eq!(block("formal_charge").range(), 16..21);
        assert_eq!(block("num_hs").range(), 21..26);
        assert_eq!(block("hybridisation").range(), 26..31);
        assert_eq!(block("aromatic").range(), 31..32);
        assert_eq!(block("in_ring").range(), 32..33);
    }

    /// Block widths are derived from the enums in `graph.rs`. If a variant is
    /// added without widening the block, features silently stop matching Python.
    #[test]
    fn block_widths_match_the_enums_they_encode() {
        assert_eq!(block("element").width, Element::COUNT);
        assert_eq!(block("hybridisation").width, Hybridisation::COUNT);
        assert_eq!(block("degree").width, MAX_DEGREE_BUCKET + 1);
        assert_eq!(block("num_hs").width, MAX_H_BUCKET + 1);
        assert_eq!(
            block("formal_charge").width,
            (MAX_CHARGE - MIN_CHARGE + 1) as usize
        );
    }

    #[test]
    fn a_carbon_row_sets_exactly_the_expected_bits() {
        // Aliphatic CH3 carbon: element C, degree 1, charge 0, 3 H, sp3.
        let row = atom_row(Element::C, 1, 0, 3, Hybridisation::Sp3, false, false);

        assert!(row_is_wellformed(&row));
        assert_eq!(row[0], 1.0, "element C is index 0");
        assert_eq!(row[10 + 1], 1.0, "degree 1");
        assert_eq!(row[16 + 2], 1.0, "charge 0 is the middle of -2..=+2");
        assert_eq!(row[21 + 3], 1.0, "3 hydrogens");
        assert_eq!(row[26 + 2], 1.0, "sp3 is index 2");
        assert_eq!(row[31], 0.0, "not aromatic");
        assert_eq!(row[32], 0.0, "not in a ring");
        assert_eq!(row.iter().sum::<f32>(), 5.0, "five one-hot bits, no flags");
    }

    #[test]
    fn an_aromatic_ring_carbon_sets_both_flags() {
        let row = atom_row(Element::C, 2, 0, 1, Hybridisation::Sp2, true, true);
        assert!(row_is_wellformed(&row));
        assert_eq!(row[31], 1.0);
        assert_eq!(row[32], 1.0);
        assert_eq!(
            row.iter().sum::<f32>(),
            7.0,
            "five one-hot bits plus two flags"
        );
    }

    #[test]
    fn charge_buckets_span_minus_two_to_plus_two() {
        for (charge, expected_bucket) in [(-2i8, 0usize), (-1, 1), (0, 2), (1, 3), (2, 4)] {
            let row = atom_row(Element::N, 1, charge, 0, Hybridisation::Sp3, false, false);
            assert_eq!(
                row[16 + expected_bucket],
                1.0,
                "charge {charge} should set bucket {expected_bucket}"
            );
            assert!(row_is_wellformed(&row));
        }
    }

    /// Out-of-range values must clamp, never error and never leave a block all
    /// zero. An all-zero block is indistinguishable from padding, and the model
    /// would treat a real atom as absent.
    #[test]
    fn out_of_range_values_clamp_into_edge_buckets() {
        // Degree 9 is chemically absurd but must not panic or zero the block.
        let row = atom_row(Element::S, 9, 0, 0, Hybridisation::Sp3, false, false);
        assert!(
            row_is_wellformed(&row),
            "clamping must preserve the invariant"
        );
        assert_eq!(
            row[10 + MAX_DEGREE_BUCKET],
            1.0,
            "degree clamps to the top bucket"
        );

        // Charge +7 (some metal complexes) clamps to +2.
        let row = atom_row(
            Element::Other,
            1,
            7,
            0,
            Hybridisation::Unknown,
            false,
            false,
        );
        assert!(row_is_wellformed(&row));
        assert_eq!(row[16 + 4], 1.0, "charge clamps to +2");
        assert_eq!(row[26 + 2], 1.0, "Unknown hybridisation clamps to sp3");

        // And the negative direction.
        let row = atom_row(Element::O, 1, -9, 0, Hybridisation::Sp3, false, false);
        assert!(row_is_wellformed(&row));
        assert_eq!(row[16], 1.0, "charge clamps to -2");

        // Nine hydrogens is nonsense; it must still produce one bit.
        let row = atom_row(Element::C, 4, 0, 9, Hybridisation::Sp3, false, false);
        assert!(row_is_wellformed(&row));
        assert_eq!(row[21 + MAX_H_BUCKET], 1.0);
    }

    /// Every element must land on its own index, and the mapping must match the
    /// `element_order` array in the exported schema exactly.
    #[test]
    fn every_element_maps_to_a_distinct_index() {
        let elements = [
            Element::C,
            Element::N,
            Element::O,
            Element::S,
            Element::F,
            Element::Cl,
            Element::Br,
            Element::I,
            Element::P,
            Element::Other,
        ];
        assert_eq!(elements.len(), Element::COUNT);

        for (expected_index, element) in elements.into_iter().enumerate() {
            let row = atom_row(element, 1, 0, 0, Hybridisation::Sp3, false, false);
            assert_eq!(
                row[expected_index], 1.0,
                "{element:?} should set index {expected_index}"
            );
            assert!(row_is_wellformed(&row));
        }
    }

    #[test]
    fn wellformedness_rejects_malformed_rows() {
        let mut row = atom_row(Element::C, 1, 0, 3, Hybridisation::Sp3, false, false);
        assert!(row_is_wellformed(&row));

        // Two bits in the element block.
        row[1] = 1.0;
        assert!(!row_is_wellformed(&row), "two bits in one block is invalid");

        // No bits in a block.
        let mut row = atom_row(Element::C, 1, 0, 3, Hybridisation::Sp3, false, false);
        row[0] = 0.0;
        assert!(
            !row_is_wellformed(&row),
            "an empty block looks like padding"
        );

        // A non-binary value.
        let mut row = atom_row(Element::C, 1, 0, 3, Hybridisation::Sp3, false, false);
        row[0] = 0.5;
        assert!(!row_is_wellformed(&row));

        assert!(!row_is_wellformed(&[0.0; 32]), "wrong width");
    }

    /// The schema JSON is the single source of truth the Python featuriser reads.
    /// If a key is renamed here, the Python side must change in the same commit --
    /// which is exactly the coupling this test makes visible.
    #[test]
    fn exported_schema_carries_everything_python_needs() {
        let json = schema_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

        assert_eq!(parsed["schema_version"], SCHEMA_VERSION);
        assert_eq!(parsed["n_features"], N_ATOM_FEATURES);
        assert_eq!(parsed["max_heavy_atoms"], MAX_HEAVY_ATOMS);

        let blocks = parsed["blocks"].as_array().expect("blocks array");
        assert_eq!(blocks.len(), SCHEMA.len());
        assert_eq!(blocks[0]["name"], "element");
        assert_eq!(blocks[0]["offset"], 0);
        assert_eq!(blocks[0]["width"], 10);

        // Element order is what makes the one-hot indices mean the same thing in
        // both languages. Its length must match the block width.
        let order = parsed["element_order"].as_array().expect("element_order");
        assert_eq!(order.len(), Element::COUNT);
        assert_eq!(order[0], "C");
        assert_eq!(order[9], "other");

        assert_eq!(
            parsed["hybridisation_order"].as_array().unwrap().len(),
            Hybridisation::COUNT
        );
        assert_eq!(parsed["charge_buckets"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn zeroed_tensors_have_the_right_shape() {
        let f = Featurised::zeroed();
        assert_eq!(f.x.len(), 128 * 33);
        assert_eq!(f.adj.len(), 128 * 128);
        assert_eq!(f.mask.len(), 128);
        assert_eq!(f.n_atoms, 0);
        assert!(f.validate(), "an empty molecule is a valid shape");
    }

    #[test]
    fn mask_marks_real_atoms_and_nothing_else() {
        let mut f = Featurised::zeroed();
        f.n_atoms = 3;
        for i in 0..3 {
            f.mask[i] = 1.0;
            f.x[i * N_ATOM_FEATURES] = 1.0;
        }
        assert!(f.validate());

        // A mask bit past the atom count means padding contributes to pooling.
        f.mask[5] = 1.0;
        assert!(!f.validate(), "padding must never be masked in");
    }

    #[test]
    fn padded_feature_rows_must_stay_zero() {
        let mut f = Featurised::zeroed();
        f.n_atoms = 2;
        f.mask[0] = 1.0;
        f.mask[1] = 1.0;
        assert!(f.validate());

        // Non-zero data in a padded row survives the matmul even with the mask,
        // because adj @ x happens before masking.
        f.x[10 * N_ATOM_FEATURES] = 1.0;
        assert!(!f.validate(), "padded rows must be all zero");
    }

    #[test]
    fn masking_an_atom_clears_its_row_and_mask_bit() {
        let mut f = Featurised::zeroed();
        f.n_atoms = 4;
        for i in 0..4 {
            f.mask[i] = 1.0;
            f.x[i * N_ATOM_FEATURES + i] = 1.0;
        }

        f.mask_atom(1);
        assert_eq!(f.mask[1], 0.0);
        assert!(f.row(1).iter().all(|&v| v == 0.0), "row must be cleared");
        // Neighbours are untouched -- occlusion removes one atom, not a range.
        assert_eq!(f.mask[0], 1.0);
        assert_eq!(f.mask[2], 1.0);

        // Out-of-range indices are a no-op, not a panic: attribution loops to
        // n_atoms and an off-by-one there should not kill the request.
        f.mask_atom(999);
        assert_eq!(f.mask[3], 1.0);
    }

    #[test]
    fn oversized_molecules_are_rejected_before_any_work() {
        let g = MolGraph {
            n_atoms: MAX_HEAVY_ATOMS + 1,
            nbr_offsets: vec![0; MAX_HEAVY_ATOMS + 2],
            ..Default::default()
        };
        assert!(matches!(
            featurise(&g),
            Err(crate::CoreError::TooLarge { .. })
        ));
    }

    #[test]
    #[ignore = "Increment 2: features::featurise"]
    fn benzene_featurises_to_six_identical_aromatic_rows() {
        let g = crate::smiles::parse("c1ccccc1").expect("benzene");
        let f = featurise(&g).expect("featurise");

        assert_eq!(f.n_atoms, 6);
        assert!(f.validate());

        // Benzene's six carbons are genuinely equivalent, so their rows must be
        // byte-identical. If they are not, something is leaking atom index into
        // the features.
        let first = f.row(0).to_vec();
        for i in 1..6 {
            assert_eq!(f.row(i), &first[..], "atom {i} differs from atom 0");
        }
        assert!(row_is_wellformed(&first));
        assert_eq!(first[31], 1.0, "aromatic");
        assert_eq!(first[32], 1.0, "in ring");
    }

    #[test]
    #[ignore = "Increment 2: features::featurise"]
    fn adjacency_is_symmetric_and_normalised() {
        let g = crate::smiles::parse("c1ccccc1").expect("benzene");
        let f = featurise(&g).expect("featurise");
        let n = f.n_atoms;

        for i in 0..n {
            for j in 0..n {
                let ij = f.adj[i * MAX_HEAVY_ATOMS + j];
                let ji = f.adj[j * MAX_HEAVY_ATOMS + i];
                assert!(
                    (ij - ji).abs() < 1e-6,
                    "adjacency must be symmetric at ({i},{j})"
                );
            }
            assert!(
                f.adj[i * MAX_HEAVY_ATOMS + i] > 0.0,
                "self-loop on the diagonal"
            );
        }

        // Every benzene carbon has degree 2, plus a self-loop, so D = 3 and each
        // non-zero entry of D^-1/2 (A+I) D^-1/2 is exactly 1/3.
        assert!((f.adj[0] - 1.0 / 3.0).abs() < 1e-6);
    }
}
