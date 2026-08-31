//! The ring-closure table.
//!
//! Manual chapter 7.3. This is the one genuinely clever part of the parser, so it
//! gets its own module.
//!
//! # What a ring digit means
//!
//! A digit means *"there is a bond here to a partner atom marked with the same
//! digit"*. The digit is a **temporary label**, freed and reusable the moment its
//! pair closes — `c1ccccc1C1CCCCC1` validly uses `1` twice, and a parser that
//! treats digits as globally unique rejects perfectly good input.
//!
//! # Why a fixed array of 100 slots
//!
//! `%nn` extends labels to 99, so 100 slots covers the entire grammar. A fixed
//! array is exactly right here:
//!
//! - `O(1)` lookup with no hashing
//! - no allocation, ever — it lives on the stack
//! - `100 × 8` bytes fits in a couple of cache lines
//!
//! A `HashMap<u8, _>` would be slower, allocate, and buy nothing: the key space
//! is small, dense, and known at compile time. Reaching for a hash map by reflex
//! when an array fits is a habit worth noticing.

use crate::graph::BondKind;
use crate::smiles::SmilesError;

/// Number of ring-closure labels the grammar permits: `0`–`9` and `%10`–`%99`.
pub const RING_SLOTS: usize = 100;

/// Pending ring bonds, indexed by label.
///
/// `Some((atom, kind))` means that label was opened at `atom` and is awaiting its
/// partner. `None` means the label is free.
#[derive(Debug, Clone)]
pub struct RingTable {
    slots: [Option<(u32, BondKind)>; RING_SLOTS],
}

impl Default for RingTable {
    fn default() -> Self {
        Self::new()
    }
}

impl RingTable {
    /// An empty table. No allocation.
    pub fn new() -> Self {
        Self {
            slots: [None; RING_SLOTS],
        }
    }

    /// Whether `label` currently has an open bond.
    #[inline]
    pub fn is_open(&self, label: u8) -> bool {
        self.slots[label as usize].is_some()
    }

    /// Record the first occurrence of `label`, opened at `atom`.
    ///
    /// Overwrites any existing entry. The caller checks [`RingTable::is_open`]
    /// first when it needs to distinguish opening from closing — which
    /// [`RingTable::take`] does.
    #[inline]
    pub fn open(&mut self, label: u8, atom: u32, kind: BondKind) {
        self.slots[label as usize] = Some((atom, kind));
    }

    /// Take the pending bond for `label`, freeing the slot for reuse.
    #[inline]
    pub fn take(&mut self, label: u8) -> Option<(u32, BondKind)> {
        self.slots[label as usize].take()
    }

    /// The lowest label still open, if any.
    ///
    /// Called once at end of input. A label still open means a ring was never
    /// closed, which is [`SmilesError::UnclosedRing`] — and silently accepting it
    /// produces a graph missing a bond, i.e. a different molecule, predicted
    /// confidently.
    pub fn first_unclosed(&self) -> Option<u8> {
        self.slots.iter().position(Option::is_some).map(|i| i as u8)
    }

    /// Reconcile bond orders stated at the two ends of a ring closure.
    ///
    /// The order may be written at either end, or both. `C=1CCCCC1` and
    /// `C1CCCCC=1` are the same molecule, so both ends are consulted:
    ///
    /// | Opened as | Closed as | Result |
    /// |---|---|---|
    /// | `Unspecified` | anything | the closing order |
    /// | anything | `Unspecified` | the opening order |
    /// | `X` | `X` | `X` |
    /// | `X` | `Y` | [`SmilesError::RingBondMismatch`] |
    ///
    /// That last row is a real error, not something to paper over by preferring
    /// one end. `C=1CCCCC#1` states two different bond orders for one bond, and
    /// guessing which the chemist meant is worse than asking.
    pub fn reconcile(
        label: u8,
        opened: BondKind,
        closed: Option<BondKind>,
    ) -> Result<BondKind, SmilesError> {
        match (opened, closed) {
            (BondKind::Unspecified, Some(c)) => Ok(c),
            (o, None) => Ok(o),
            (o, Some(c)) if o == c => Ok(o),
            (o, Some(c)) => Err(SmilesError::RingBondMismatch {
                digit: label,
                first: o,
                second: c,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_freed_for_reuse_when_closed() {
        let mut t = RingTable::new();
        t.open(1, 0, BondKind::Unspecified);
        assert!(t.is_open(1));

        assert_eq!(t.take(1), Some((0, BondKind::Unspecified)));
        assert!(!t.is_open(1), "closing a label must free it");

        // The whole point: digit 1 is usable again immediately, which is what
        // makes c1ccccc1C1CCCCC1 legal.
        t.open(1, 6, BondKind::Unspecified);
        assert_eq!(t.take(1), Some((6, BondKind::Unspecified)));
    }

    #[test]
    fn two_digit_labels_are_addressable() {
        let mut t = RingTable::new();
        t.open(99, 42, BondKind::Double);
        assert_eq!(t.take(99), Some((42, BondKind::Double)));
    }

    #[test]
    fn unclosed_labels_are_reported_lowest_first() {
        let mut t = RingTable::new();
        assert_eq!(t.first_unclosed(), None);

        t.open(7, 1, BondKind::Unspecified);
        t.open(2, 3, BondKind::Unspecified);
        assert_eq!(
            t.first_unclosed(),
            Some(2),
            "lowest open label, not insertion order"
        );

        t.take(2);
        assert_eq!(t.first_unclosed(), Some(7));
        t.take(7);
        assert_eq!(t.first_unclosed(), None);
    }

    #[test]
    fn bond_order_may_be_stated_at_either_end() {
        // Stated only at close.
        assert_eq!(
            RingTable::reconcile(1, BondKind::Unspecified, Some(BondKind::Double)).unwrap(),
            BondKind::Double
        );
        // Stated only at open.
        assert_eq!(
            RingTable::reconcile(1, BondKind::Double, None).unwrap(),
            BondKind::Double
        );
        // Stated at both, in agreement.
        assert_eq!(
            RingTable::reconcile(1, BondKind::Double, Some(BondKind::Double)).unwrap(),
            BondKind::Double
        );
        // Neither end stated an order.
        assert_eq!(
            RingTable::reconcile(1, BondKind::Unspecified, None).unwrap(),
            BondKind::Unspecified
        );
    }

    #[test]
    fn contradictory_bond_orders_are_an_error_not_a_guess() {
        let err = RingTable::reconcile(3, BondKind::Double, Some(BondKind::Triple)).unwrap_err();
        assert!(matches!(
            err,
            SmilesError::RingBondMismatch {
                digit: 3,
                first: BondKind::Double,
                second: BondKind::Triple
            }
        ));
    }

    #[test]
    fn table_covers_the_whole_label_space_without_panicking() {
        let mut t = RingTable::new();
        for label in 0..RING_SLOTS as u8 {
            t.open(label, label as u32, BondKind::Single);
            assert!(t.is_open(label));
        }
        assert_eq!(t.first_unclosed(), Some(0));
        for label in 0..RING_SLOTS as u8 {
            assert_eq!(t.take(label), Some((label as u32, BondKind::Single)));
        }
        assert_eq!(t.first_unclosed(), None);
    }
}
