//! The persistent prediction cache.
//!
//! Manual chapter 20.3. Two layers sit in front of PostgreSQL: the sharded LRU
//! in `admet-infer` (survives a request), and this (survives a restart). Neither
//! is optional -- a cold cache after a deployment turns a 12 ms cache hit back
//! into a 40 ms inference for every molecule the service has ever seen.

use sqlx::PgPool;
use uuid::Uuid;

use crate::model::{Prediction, PredictionValue};
use crate::{DbError, Result};

/// Queries over `predictions` and `prediction_values`.
#[derive(Debug, Clone)]
pub struct PredictionRepo {
    pool: PgPool,
}

/// A prediction with its twelve endpoint values, as the API returns it.
///
/// A separate type from [`Prediction`] because the table row and the response
/// payload are different things that happen to overlap. Returning row structs
/// straight from handlers couples the wire format to the schema, and then a
/// column rename becomes a breaking API change.
#[derive(Debug, Clone)]
pub struct FullPrediction {
    /// The prediction row.
    pub prediction: Prediction,
    /// Its endpoint values, ordered by [`crate::model::Endpoint::output_index`]
    /// so the twelve always appear in the same order as the model's output
    /// tensor. Ordering in SQL rather than in the client means the UI cannot
    /// accidentally reorder them per request.
    pub values: Vec<PredictionValue>,
}

impl PredictionRepo {
    /// Wrap a pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The underlying pool, for callers running these queries in their own
    /// transaction.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Cache probe: the prediction for this molecule under this model version.
    ///
    /// The `(molecule_id, model_version_id)` unique index makes this an index
    /// lookup returning at most one row. Including the model version in the key
    /// is ADR-04's load-bearing detail: after a model upgrade, old entries are
    /// unreachable rather than wrong, so no invalidation sweep is needed and
    /// there is no window in which the service serves numbers from a model that
    /// is no longer deployed.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 2.
    pub async fn find_cached(
        &self,
        molecule_id: Uuid,
        model_version_id: Uuid,
    ) -> Result<Option<FullPrediction>> {
        let _ = (molecule_id, model_version_id);
        Err(DbError::NotImplemented("PredictionRepo::find_cached"))
    }

    /// Store a prediction and its values atomically.
    ///
    /// One transaction, two statements. Committing the parent row before its
    /// values would leave a window in which a concurrent read finds a prediction
    /// with zero endpoints -- and the response type has no way to express that,
    /// so it would render as twelve blanks rather than as an error.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 2.
    pub async fn insert(&self, full: &FullPrediction) -> Result<Uuid> {
        let _ = full;
        Err(DbError::NotImplemented("PredictionRepo::insert"))
    }

    /// Every prediction in a batch, ranked by triage score, best first.
    ///
    /// `ORDER BY triage_score DESC NULLS LAST` -- the `NULLS LAST` is the whole
    /// clause. A withheld score is `NULL` (FR-12), PostgreSQL sorts `NULL` as
    /// largest under `DESC` by default, and the default would therefore put every
    /// out-of-domain compound at the *top* of the ranked table. That is the worst
    /// possible failure for this feature: it recommends precisely the molecules
    /// the model said it could not judge.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 2.
    pub async fn top_for_batch(&self, batch_id: Uuid, limit: i64) -> Result<Vec<FullPrediction>> {
        let _ = (batch_id, limit);
        Err(DbError::NotImplemented("PredictionRepo::top_for_batch"))
    }

    /// Latency percentiles over a recent window, for the NFR-01 evidence table.
    ///
    /// `percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms)` computed in the
    /// database, not by pulling a million rows into the process and sorting them.
    /// Returns `(p50, p95, p99, count)` in milliseconds.
    ///
    /// The count is returned alongside deliberately: a p99 over 40 requests is
    /// not a p99, and reporting one without its sample size is the kind of thing
    /// an examiner asks about.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 2.
    pub async fn latency_percentiles(&self, since_hours: i32) -> Result<(f64, f64, f64, i64)> {
        let _ = since_hours;
        Err(DbError::NotImplemented(
            "PredictionRepo::latency_percentiles",
        ))
    }

    /// Count by domain status, for the "how much of this batch could we actually
    /// judge" figure.
    ///
    /// Returns `(in_domain, low_confidence, out_of_domain)`. Worth surfacing in
    /// the UI: a batch that is 60% out of domain has been screened against
    /// chemistry the model has never seen, and the honest reading of that result
    /// is "unknown", not "unpromising".
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 2.
    pub async fn domain_breakdown(&self, batch_id: Uuid) -> Result<(i64, i64, i64)> {
        let _ = batch_id;
        Err(DbError::NotImplemented("PredictionRepo::domain_breakdown"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FullPrediction` is the response shape, so the field names are part of
    /// the API surface. This test does nothing clever; it exists so that
    /// renaming a field breaks something and prompts a version bump.
    #[test]
    fn full_prediction_pairs_a_row_with_its_values() {
        fn _shape(f: &FullPrediction) -> (&Prediction, usize) {
            (&f.prediction, f.values.len())
        }
    }

    #[test]
    fn unimplemented_methods_report_their_own_name() {
        let e = DbError::NotImplemented("PredictionRepo::find_cached");
        assert!(e.to_string().contains("find_cached"));
    }
}
