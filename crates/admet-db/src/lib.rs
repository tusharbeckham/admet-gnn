//! Persistence for ADMETriage.
//!
//! Manual chapters 11 (ER model), 12 (normalisation) and 20 (`sqlx` in
//! practice). Everything that touches PostgreSQL lives here and nowhere else.
//!
//! # What this crate is for
//!
//! Three jobs, in order of how much they matter:
//!
//! 1. **Identity and deduplication.** One row per molecule, keyed by InChIKey.
//!    A batch of 10,000 rows from a chemist typically contains a few hundred
//!    duplicates; catching them here is the difference between 10,000 model
//!    invocations and 9,600.
//! 2. **The persistent prediction cache.** Keyed `(molecule, model_version)`.
//!    The in-process LRU in `admet-infer` sits in front of it and survives a
//!    request; this survives a restart.
//! 3. **Provenance.** Which model version produced which number, and when. A
//!    prediction with no recorded model version is not reproducible, and an
//!    irreproducible number cannot go in the report.
//!
//! # Scaffold status
//!
//! Types and repository signatures are here; the SQL is not. Increment 2 writes
//! `migrations/0001_initial.sql` and fills in the bodies. Until then every
//! repository method returns [`DbError::NotImplemented`], which is an `Err` --
//! so callers compile against the real signature and nothing silently returns
//! an empty result set that looks like a cache miss.
//!
//! # A note on the query macros
//!
//! `sqlx::query!` checks SQL against a live database *at compile time*, which is
//! genuinely excellent and the main reason to pick `sqlx` over an ORM. It also
//! means the crate does not compile without either a reachable `DATABASE_URL`
//! or a committed `.sqlx/` offline cache. That trade is worth taking in
//! Increment 2 -- run `cargo sqlx prepare` and commit `.sqlx/` so CI does not
//! need a database to type-check queries. It is *not* worth taking in the
//! scaffold, where there is no schema to check against, so nothing below uses
//! the macros yet.

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod model;
pub mod repository;

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

//  Re-exported so that callers -- `admet-api` in particular -- can name the
//  pool type without taking a direct `sqlx` dependency. ADR-02 puts the
//  database driver behind this crate's boundary; if every consumer has to
//  `use sqlx::PgPool` to hold a handle, that boundary exists only on paper.
pub use sqlx::postgres::PgPool as Pool;

/// Where migrations live, relative to the repository root.
///
/// Root-level rather than `crates/admet-db/migrations/`, which is what
/// `implementation.md` §8 sketched. Reason: `sqlx-cli`, `docker compose` and CI
/// all resolve `./migrations` from the repository root, where `.env` and
/// `DATABASE_URL` also live. Keeping the directory where the tools already look
/// avoids passing `--source` to every invocation, and one path is easier to keep
/// correct than two.
pub const MIGRATIONS_DIR: &str = "migrations";

/// Default connection-pool size.
///
/// Ten, not "as many as possible". PostgreSQL allocates roughly 10 MB per
/// backend and its default `max_connections` is 100, so a pool that grows
/// without bound turns a traffic spike into `FATAL: too many clients` -- an
/// outage caused by the client, not the database. The API is CPU-bound on
/// inference anyway; connections spend most of their life idle.
pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;

/// How long to wait for a free connection before giving up.
///
/// Bounded on purpose. An unbounded acquire turns pool exhaustion into requests
/// that hang forever, which is harder to diagnose than requests that fail: a
/// timeout produces a log line and a 503, a hang produces a confused user.
pub const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors this crate returns.
///
/// `thiserror`, not `anyhow`: this is a library, and the API layer needs to
/// distinguish "not found" (404) from "unique violation" (409) from "the
/// database is down" (503). Collapsing those into one opaque error means every
/// failure becomes a 500 and the client cannot tell a mistake from an outage.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// The underlying driver failed: connection lost, syntax error, constraint
    /// violation. Inspect [`sqlx::Error`] to classify further.
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// A migration failed to apply. Almost always a checksum mismatch, meaning
    /// someone edited an already-applied migration file instead of adding a new
    /// one. `sqlx` refusing to continue here is correct behaviour.
    #[error("migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// The row does not exist. Separate from [`DbError::Sqlx`] because the API
    /// maps this to 404 and everything else to 5xx.
    #[error("{entity} not found: {key}")]
    NotFound {
        /// Table or logical entity name, for the log line.
        entity: &'static str,
        /// The key that was looked up.
        key: String,
    },

    /// A stored value failed to convert back into a domain type -- an InChIKey
    /// column holding something that is not a valid InChIKey, for instance.
    /// Indicates data written by something that bypassed this crate.
    #[error("stored value is not valid for the domain type: {0}")]
    Corrupt(String),

    /// Scaffold placeholder. **Delete this variant** once every repository
    /// method has a body; while it exists, `cargo build` naming it is a useful
    /// reminder of what is left.
    #[error("not implemented yet: {0}")]
    NotImplemented(&'static str),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, DbError>;

/// Connect and build a pool.
///
/// Fails fast rather than lazily: the pool acquires one connection up front, so
/// a bad `DATABASE_URL` surfaces at start-up with a clear message instead of on
/// the first request an hour later.
///
/// # Errors
/// [`DbError::Sqlx`] if the URL is malformed or the server is unreachable.
pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(DEFAULT_MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Apply every pending migration.
///
/// Reads `.sql` files from [`MIGRATIONS_DIR`] **at run time**, which is why this
/// takes no macro. That is a deliberate scaffold choice: `sqlx::migrate!()`
/// embeds the files into the binary at compile time, and embedding is what you
/// want for the distroless Docker image in Increment 5 -- a container that must
/// also carry a `migrations/` directory is a container with two things to keep
/// in sync. Swap to:
///
/// ```ignore
/// static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");
/// ```
///
/// once `0001_initial.sql` exists.
///
/// Applying migrations at start-up is fine for one instance and wrong for
/// several: concurrent instances race on the lock and the slow ones time out.
/// When there is more than one replica, this moves to a deploy step.
///
/// # Errors
/// [`DbError::Migration`] on a checksum mismatch or failing statement.
pub async fn migrate(pool: &PgPool) -> Result<()> {
    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new(MIGRATIONS_DIR)).await?;
    migrator.run(pool).await?;
    Ok(())
}

/// Cheap liveness probe for `/healthz`.
///
/// `SELECT 1` and nothing more. A health check that runs a real query measures
/// the query, and a health check that touches application tables will report
/// unhealthy during a long migration -- which then removes the instance from
/// the load balancer exactly when it is doing necessary work.
///
/// # Errors
/// [`DbError::Sqlx`] if the round trip fails.
pub async fn ping(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pool limits are a deliberate number with a reason, not a default that
    /// happened to be there. If someone raises this, the comment on the
    /// constant is the argument they have to answer.
    #[test]
    fn pool_limits_are_bounded() {
        const {
            assert!(
                DEFAULT_MAX_CONNECTIONS <= 20,
                "a pool this size starves PostgreSQL"
            )
        };
        assert!(
            ACQUIRE_TIMEOUT <= Duration::from_secs(10),
            "waits must be bounded"
        );
    }

    /// The API's error mapping is a `match` on this enum, so the variants have
    /// to stay distinguishable. Collapsing `NotFound` into `Sqlx` would turn
    /// every 404 into a 500 and nothing would fail loudly.
    #[test]
    fn not_found_is_distinguishable_from_a_driver_error() {
        let e = DbError::NotFound {
            entity: "molecule",
            key: "BSYNRYMUTXBXSQ-UHFFFAOYSA-N".to_owned(),
        };
        assert!(matches!(e, DbError::NotFound { .. }));
        assert!(
            e.to_string().contains("BSYNRYMUTXBXSQ"),
            "the key must be in the message"
        );
    }
}
