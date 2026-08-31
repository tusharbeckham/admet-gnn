//! Connected components, ring perception, and Bemis–Murcko scaffolds.
//!
//! Manual chapter 9. [`UnionFind`] is **fully implemented**; the functions that
//! need a parsed [`MolGraph`] are stubs.
//!
//! # Why this module decides whether your results are honest
//!
//! [`murcko_scaffold`] is the most important algorithm in the crate that has
//! nothing to do with speed. It determines whether your reported accuracy means
//! anything.
//!
//! Chemical datasets contain **analogue series** — twenty variations on one
//! scaffold. A random split scatters them across train and test, so the model sees
//! close cousins of every test compound and effectively memorises the scaffold.
//! Reported accuracy can be **10–20 percentage points optimistic**. A scaffold
//! split forces generalisation to unseen chemistry, which is the only question a
//! chemist actually cares about: *will this work on my new compound?*
//!
//! The split itself is implemented on the Python side
//! (`training/data/scaffold_split.py`) because that is where training data is
//! partitioned. This module provides the Rust-side scaffold extraction used for
//! reporting and for grouping batch results. Both must agree.

use crate::graph::MolGraph;

/// Disjoint-set union with path halving and union by rank.
///
/// Complexity is `O(E · α(N))`, where α is the inverse Ackermann function —
/// below 5 for any input that fits in the universe. Treat it as linear.
#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<u32>,
    rank: Vec<u8>,
    /// Number of disjoint sets remaining. Maintained incrementally so
    /// [`UnionFind::set_count`] is `O(1)` rather than a scan.
    sets: usize,
}

impl UnionFind {
    /// `n` singleton sets.
    pub fn new(n: usize) -> Self {
        Self {
            parent: (0..n as u32).collect(),
            rank: vec![0; n],
            sets: n,
        }
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether there are no elements.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Representative of `x`'s set, compressing the path on the way.
    ///
    /// **Path halving**, not full path compression: each visited node is pointed
    /// at its grandparent. Same asymptotic guarantee, one pass instead of two, and
    /// no recursion — so a pathological chain cannot overflow the stack.
    pub fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let grandparent = self.parent[self.parent[x as usize] as usize];
            self.parent[x as usize] = grandparent;
            x = grandparent;
        }
        x
    }

    /// Join the sets containing `a` and `b`.
    ///
    /// Returns `false` when they were already joined — which means this edge
    /// **closes a cycle**. See [`UnionFind::set_count`] and the free-bonus note
    /// on [`cycle_rank`].
    ///
    /// Union by rank keeps the tree shallow, which is what makes the α(N) bound
    /// hold; joining arbitrarily degrades to a linked list.
    pub fn union(&mut self, a: u32, b: u32) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        let (hi, lo) = if self.rank[ra as usize] >= self.rank[rb as usize] {
            (ra, rb)
        } else {
            (rb, ra)
        };
        self.parent[lo as usize] = hi;
        if self.rank[hi as usize] == self.rank[lo as usize] {
            self.rank[hi as usize] += 1;
        }
        self.sets -= 1;
        true
    }

    /// Whether `a` and `b` are in the same set.
    pub fn connected(&mut self, a: u32, b: u32) -> bool {
        self.find(a) == self.find(b)
    }

    /// Number of disjoint sets. `O(1)`.
    pub fn set_count(&self) -> usize {
        self.sets
    }

    /// Group elements by representative, as a list of member lists.
    ///
    /// Groups are returned in ascending order of their lowest member, so the
    /// output is deterministic — which matters because the caller picks "the
    /// largest fragment" and needs ties broken the same way every run.
    pub fn groups(&mut self) -> Vec<Vec<u32>> {
        let n = self.len();
        let mut by_root: std::collections::BTreeMap<u32, Vec<u32>> =
            std::collections::BTreeMap::new();
        for i in 0..n as u32 {
            let root = self.find(i);
            by_root.entry(root).or_default().push(i);
        }
        let mut groups: Vec<Vec<u32>> = by_root.into_values().collect();
        groups.sort_by_key(|g| g[0]);
        groups
    }
}

/// Connected components of a molecular graph.
///
/// Salts and mixtures arrive as multi-component SMILES: `CC(=O)O.[Na+]` is sodium
/// acetate. You must detect this, because predicting properties **for a mixture is
/// meaningless** — the policy is to keep the largest organic fragment and warn.
///
/// # Errors
/// [`crate::CoreError::NotImplemented`] until Increment 2.
pub fn components(graph: &MolGraph) -> crate::Result<Vec<Vec<u32>>> {
    let _ = graph;
    Err(crate::CoreError::NotImplemented("scaffold::components"))
}

/// Number of independent rings, via the circuit-rank formula `E − N + C`.
///
/// # The free bonus
///
/// [`UnionFind::union`] returns `false` exactly when an edge closes a cycle.
/// Counting those `false` returns while computing components gives the cycle rank
/// **in the same pass, at no extra cost**. Getting a second useful result out of
/// an algorithm you already needed is a nice detail to point out.
///
/// Note what this does and does not tell you: knowing *how many* rings exist is
/// easy. Knowing *which atoms* form them requires a cycle basis — see
/// [`ring_membership`].
///
/// # Errors
/// [`crate::CoreError::NotImplemented`] until Increment 2.
pub fn cycle_rank(graph: &MolGraph) -> crate::Result<usize> {
    let _ = graph;
    Err(crate::CoreError::NotImplemented("scaffold::cycle_rank"))
}

/// Per-atom ring membership, feeding feature index 32.
///
/// # Approach, and why not SSSR
///
/// | Approach | Complexity | When to use |
/// |---|---|---|
/// | Cycle rank only | `O(E·α(N))` | You just need a ring count. Often enough |
/// | **BFS from each atom** | `O(N·E)` | Ring membership per atom. Fine at N≤128 |
/// | Horton / SSSR | `O(N³)` typical | You need actual ring systems for scaffolds |
///
/// BFS per atom is chosen: at N ≤ 128 the cubic option buys nothing, and the
/// feature vector only needs a boolean.
///
/// **SSSR is genuinely ambiguous.** For fused systems like cubane, the smallest
/// set of smallest rings is not unique — different valid answers exist. RDKit
/// exposes `GetSymmSSSR` for a symmetrised version. That is a known wart in
/// cheminformatics, not a bug in your code; mentioning it shows you read the
/// domain literature rather than just the API docs.
///
/// # Errors
/// [`crate::CoreError::NotImplemented`] until Increment 2.
pub fn ring_membership(graph: &MolGraph) -> crate::Result<Vec<bool>> {
    let _ = graph;
    Err(crate::CoreError::NotImplemented(
        "scaffold::ring_membership",
    ))
}

/// The Bemis–Murcko scaffold: the molecule's core framework.
///
/// Strip all side chains, keep ring systems and the linkers between them. Aspirin
/// and paracetamol both reduce to a benzene scaffold.
///
/// # Algorithm — iterative terminal-atom pruning
///
/// Repeatedly delete every atom with degree 1 that is not in a ring. Stop when
/// nothing changes. What remains is the scaffold. Cost is `O(N · deg)`, because
/// each atom is removed at most once.
///
/// Returns the atom indices that survive, so the caller can either build a
/// subgraph or use the set directly for grouping.
///
/// # Errors
/// [`crate::CoreError::NotImplemented`] until Increment 2.
pub fn murcko_scaffold(graph: &MolGraph) -> crate::Result<Vec<u32>> {
    let _ = graph;
    Err(crate::CoreError::NotImplemented(
        "scaffold::murcko_scaffold",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singletons_start_disconnected() {
        let mut uf = UnionFind::new(5);
        assert_eq!(uf.len(), 5);
        assert_eq!(uf.set_count(), 5);
        for i in 0..5u32 {
            for j in 0..5u32 {
                assert_eq!(uf.connected(i, j), i == j);
            }
        }
    }

    #[test]
    fn union_merges_and_is_idempotent() {
        let mut uf = UnionFind::new(6);
        assert!(uf.union(0, 1), "first join reports a merge");
        assert_eq!(uf.set_count(), 5);
        assert!(!uf.union(0, 1), "re-joining reports no merge");
        assert_eq!(
            uf.set_count(),
            5,
            "a failed union must not change the count"
        );
        assert!(uf.connected(0, 1));
        assert!(!uf.connected(0, 2));
    }

    #[test]
    fn union_is_transitive() {
        let mut uf = UnionFind::new(5);
        uf.union(0, 1);
        uf.union(1, 2);
        uf.union(3, 4);
        assert!(uf.connected(0, 2), "0-1-2 forms one set");
        assert!(!uf.connected(2, 3));
        assert_eq!(uf.set_count(), 2);
        uf.union(2, 3);
        assert_eq!(uf.set_count(), 1);
        assert!(uf.connected(0, 4));
    }

    /// The property that gives cycle rank for free: `union` returns false
    /// exactly on an edge that closes a cycle. Benzene has 6 atoms and 6 bonds,
    /// so exactly one edge must be rejected -- one independent ring.
    #[test]
    fn rejected_unions_count_independent_rings() {
        let n = 6u32;
        let mut uf = UnionFind::new(n as usize);
        let mut cycle_edges = 0;
        for i in 0..n {
            if !uf.union(i, (i + 1) % n) {
                cycle_edges += 1;
            }
        }
        assert_eq!(cycle_edges, 1, "a single ring closes exactly one cycle");
        assert_eq!(uf.set_count(), 1);

        // Naphthalene: two fused rings, 10 atoms, 11 bonds -> cycle rank 2.
        let mut uf = UnionFind::new(10);
        let bonds: [(u32, u32); 11] = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 0), // first ring
            (4, 6),
            (6, 7),
            (7, 8),
            (8, 9),
            (9, 5), // fused second ring
        ];
        let mut cycles = 0;
        for (a, b) in bonds {
            if !uf.union(a, b) {
                cycles += 1;
            }
        }
        assert_eq!(cycles, 2, "two fused rings give cycle rank 2");
    }

    /// Sodium acetate: `CC(=O)O.[Na+]`. Four connected heavy atoms plus an
    /// isolated sodium. Predicting properties for the mixture is meaningless, so
    /// detecting the split is a correctness requirement, not a nicety.
    #[test]
    fn disconnected_fragments_stay_separate() {
        let mut uf = UnionFind::new(5);
        uf.union(0, 1);
        uf.union(1, 2);
        uf.union(1, 3);
        // atom 4 is the sodium: no bonds at all

        assert_eq!(uf.set_count(), 2);
        let groups = uf.groups();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0, 1, 2, 3], "the acetate fragment");
        assert_eq!(groups[1], vec![4], "the counter-ion, alone");
    }

    #[test]
    fn groups_are_deterministic_across_runs() {
        let build = || {
            let mut uf = UnionFind::new(8);
            for (a, b) in [(7u32, 3u32), (1, 5), (3, 1), (6, 2)] {
                uf.union(a, b);
            }
            uf.groups()
        };
        let first = build();
        for _ in 0..20 {
            assert_eq!(first, build(), "group order must be stable");
        }
        // Ascending by lowest member.
        assert!(first.windows(2).all(|w| w[0][0] < w[1][0]));
    }

    /// Path halving must not recurse: a 10,000-long chain would overflow the
    /// stack in a recursive implementation. Also exercises the compression.
    #[test]
    fn deep_chains_do_not_overflow() {
        let n = 10_000u32;
        let mut uf = UnionFind::new(n as usize);
        for i in 0..n - 1 {
            uf.union(i, i + 1);
        }
        assert_eq!(uf.set_count(), 1);
        assert!(uf.connected(0, n - 1));
        // After a full find, the path from the far end is short.
        let root = uf.find(n - 1);
        assert_eq!(uf.find(0), root);
    }

    #[test]
    fn empty_union_find_is_harmless() {
        let mut uf = UnionFind::new(0);
        assert!(uf.is_empty());
        assert_eq!(uf.set_count(), 0);
        assert!(uf.groups().is_empty());
    }

    #[test]
    #[ignore = "Increment 2: needs a parsed MolGraph"]
    fn aspirin_scaffold_is_benzene() {
        let g = crate::smiles::parse("CC(=O)Oc1ccccc1C(=O)O").expect("aspirin");
        let scaffold = murcko_scaffold(&g).expect("scaffold");
        assert_eq!(scaffold.len(), 6, "stripping side chains leaves the ring");

        // Paracetamol reduces to the same scaffold -- which is exactly why a
        // random split leaks: these two would end up on opposite sides.
        let p = crate::smiles::parse("CC(=O)Nc1ccc(O)cc1").expect("paracetamol");
        assert_eq!(murcko_scaffold(&p).expect("scaffold").len(), 6);
    }

    #[test]
    #[ignore = "Increment 2: needs a parsed MolGraph"]
    fn ring_membership_marks_only_ring_atoms() {
        let g = crate::smiles::parse("CC(=O)Oc1ccccc1C(=O)O").expect("aspirin");
        let in_ring = ring_membership(&g).expect("ring perception");
        assert_eq!(in_ring.iter().filter(|&&r| r).count(), 6);
        assert_eq!(cycle_rank(&g).expect("cycle rank"), 1);
    }
}
