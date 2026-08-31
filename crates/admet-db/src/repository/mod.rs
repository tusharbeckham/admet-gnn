//! Repositories: one type per aggregate, each owning its SQL.
//!
//! Manual chapter 20.3. The pattern is deliberately plain -- a struct holding a
//! [`crate::Pool`], with one method per query. No generic repository trait, no ORM.
//!
//! # Why not a trait
//!
//! A `trait Repository<T>` looks tidy and buys nothing here. There is exactly
//! one implementation of each of these, and the queries differ enough that the
//! shared abstraction would be a lowest common denominator with escape hatches.
//! The one thing a trait would buy -- swapping in a fake for tests -- is better
//! served by testing against a real PostgreSQL in a container, because the bugs
//! worth catching in this layer are SQL bugs and a fake has no SQL.
//!
//! The desktop build (Increment 5, SQLite) is the case where a trait *might* pay
//! off. Revisit it then, with the second implementation in hand, rather than
//! guessing its shape now.
//!
//! # Cloning the pool is free
//!
//! [`crate::Pool`] is an `Arc` around shared state, so each repository owns a clone
//! rather than borrowing. That keeps lifetimes out of the handler signatures in
//! `admet-api`, which is worth more than the pointer copy costs.

pub mod batch;
pub mod molecule;
pub mod prediction;

pub use batch::BatchRepo;
pub use molecule::MoleculeRepo;
pub use prediction::PredictionRepo;

/// Rows per checkpoint during a batch run.
///
/// 250 is a compromise with a number behind it on each side. Checkpoint every
/// row and 10,000 rows cost 10,000 round trips, which dominates the inference
/// they are tracking. Checkpoint only at the end and a crash at row 9,999 loses
/// everything. At 250 the write cost is 40 round trips and the worst-case loss
/// is 250 rows -- a few seconds of work. See FR-16 (results survive a reload) and
/// FR-15 (progress is streamed, so the checkpoint is also what there is to report).
pub const CHECKPOINT_INTERVAL: usize = 250;

/// Maximum rows in one `INSERT ... UNNEST` statement.
///
/// PostgreSQL's wire protocol caps a single statement at 65,535 bound
/// parameters. The `UNNEST` form binds one array per column regardless of row
/// count, so this limit is about statement size and memory rather than the
/// parameter cap -- but staying well under it means the code does not have to
/// care which limit it is near. Chunking at 1,000 keeps each statement's
/// transaction short enough not to hold locks across a long batch.
pub const BULK_INSERT_CHUNK: usize = 1_000;

#[cfg(test)]
mod tests {
    use super::*;

    /// Both constants are tuning decisions, and both have a failure mode at each
    /// extreme. The test pins the reasoning so a casual change has to argue with
    /// it.
    #[test]
    fn checkpoint_and_chunk_sizes_are_in_the_sane_band() {
        //  `const` blocks: every operand is a constant, so these are
        //  compile-time facts. A bad edit now fails the BUILD, which is a
        //  stronger guarantee than failing a test someone can `--skip`.
        const {
            assert!(
                CHECKPOINT_INTERVAL >= 50,
                "too frequent: writes dominate inference"
            )
        };
        const {
            assert!(
                CHECKPOINT_INTERVAL <= 1_000,
                "too rare: a crash loses too much work"
            )
        };
        const { assert!(BULK_INSERT_CHUNK <= 5_000, "statement size grows with this") };
        const {
            assert!(
                BULK_INSERT_CHUNK % CHECKPOINT_INTERVAL == 0,
                "a chunk should end on a checkpoint boundary, or progress reporting stutters"
            )
        };
    }
}
