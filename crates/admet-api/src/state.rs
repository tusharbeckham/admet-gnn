//! Shared application state.
//!
//! Manual chapter 19.2. One struct, cloned per request, holding handles to
//! everything a handler might need.
//!
//! # Why `Arc` and not a global
//!
//! Axum clones the state for every request, so anything in here must be cheap to
//! clone. [`admet_db::Pool`] is already an `Arc` internally; the model
//! engine is not, so it gets wrapped. The alternative -- a `static` with a
//! `OnceLock` -- works and is worse: state that arrives as a parameter can be
//! substituted in a test, and state reached through a global cannot.
//!
//! # Why the engine is `Option`
//!
//! The service must start and answer `/healthz` even when the model file is
//! missing. That sounds like tolerating a broken deployment; it is the opposite.
//! A process that exits on a missing artefact produces a crash-looping container
//! and no diagnostics, while a process that starts and reports `model: absent` on
//! `/readyz` tells the operator exactly what is wrong and stays inspectable.
//! Liveness passes, readiness fails, no traffic is routed to it, and nobody has
//! to read a container log to find out why.
//!
//! Only start-up is tolerant. A `/predict` call with no engine is a 503 with a
//! typed problem body, never a default prediction.

use std::sync::Arc;

use crate::config::Settings;

/// Handles shared by every handler.
#[derive(Clone)]
pub struct AppState {
    /// Loaded configuration. `Arc` because it is read on every request and never
    /// written after start-up.
    pub settings: Arc<Settings>,
    /// The ONNX session, absent until Increment 1 produces a `model.onnx`.
    ///
    /// The `Mutex` is not decoration. `admet_infer::Engine::run` takes
    /// `&mut self`, because an ONNX Runtime session binds its input tensors
    /// before executing, so two threads cannot share one session safely. A lock
    /// is the smallest correct thing here.
    ///
    /// It is also **temporary, and a bottleneck** -- every prediction serialises
    /// behind it, so concurrency collapses to one. Increment 2 replaces it with
    /// the design NFR-02 actually calls for: one dedicated worker task owning
    /// the engine, fed by an `mpsc` channel, coalescing up to 64 waiting requests
    /// into a single batched call. That is both faster than a lock *and* lock-free
    /// at this layer, which is why micro-batching is a performance feature rather
    /// than a complication. TR-08 requires the same channel shape for batch
    /// ingestion, so the two arrive together.
    pub engine: Option<Arc<std::sync::Mutex<admet_infer::Engine>>>,
    /// Connection pool, absent when the service started without a reachable
    /// database. Same reasoning as `engine`.
    pub db: Option<admet_db::Pool>,
    /// Process start time, for the uptime field on `/version`. Set once in
    /// `main`; a handler computing it from a lazily-initialised static would
    /// report the time of the first request instead.
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    /// State with neither model nor database, for tests and for a start-up that
    /// found neither.
    pub fn bare(settings: Settings) -> Self {
        Self {
            settings: Arc::new(settings),
            engine: None,
            db: None,
            started_at: chrono::Utc::now(),
        }
    }

    /// Whether the service can actually serve predictions.
    ///
    /// Drives `/readyz`. Both dependencies are required: an engine with no
    /// database can predict but cannot cache or record, and recording is what
    /// makes a prediction auditable rather than ephemeral.
    pub fn is_ready(&self) -> bool {
        self.engine.is_some() && self.db.is_some()
    }
}

impl std::fmt::Debug for AppState {
    /// Hand-written because `settings.database.url` contains a password, and a
    /// derived `Debug` would print it into the first log line that formats the
    /// state. Same defence as `admet_db::model::User`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("engine", &self.engine.as_ref().map(|_| "loaded"))
            .field("db", &self.db.as_ref().map(|_| "connected"))
            .field("port", &self.settings.server.port)
            .field("started_at", &self.started_at)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Readiness needs both dependencies. If this ever passes with only one, the
    /// load balancer will send traffic to an instance that 503s every request.
    #[test]
    fn readiness_requires_both_dependencies() {
        let state = AppState::bare(Settings::for_tests());
        assert!(!state.is_ready(), "no engine and no database is not ready");
        assert!(state.engine.is_none());
        assert!(state.db.is_none());
    }

    /// The connection string is a credential. It must not be printable through
    /// the state.
    #[test]
    fn debug_output_never_contains_the_database_password() {
        let mut settings = Settings::for_tests();
        //  The password is deliberately an obvious placeholder rather than
        //  something secret-shaped. This test only ever does a substring check,
        //  so realism buys nothing -- and a realistic-looking credential in
        //  committed source trips the pre-commit secret scanner (DEF-09) every
        //  time, which is how a useful scanner ends up switched off.
        settings.database.url = "postgres://admet:your-password@localhost:5432/admet".to_owned();
        let printed = format!("{:?}", AppState::bare(settings));
        assert!(
            !printed.contains("your-password"),
            "credential leaked: {printed}"
        );
    }
}
