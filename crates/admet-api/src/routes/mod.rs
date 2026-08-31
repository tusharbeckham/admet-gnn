//! HTTP routes.
//!
//! Manual chapter 19. One module per resource. The router is assembled in
//! [`build`] so `main` stays short and so integration tests can construct the
//! same router without binding a socket -- `tower::ServiceExt::oneshot` drives it
//! directly, which is why the API tests run in milliseconds and do not need a
//! free port.
//!
//! # Scaffold status
//!
//! Only the health routes exist. `/predict`, `/predict/batch` and the project
//! routes arrive in Increments 2–4; their handler signatures are sketched in
//! `predict.rs` so the shape of the request and response payloads is settled
//! before there is anything behind them.
//!
//! # Route naming
//!
//! `/healthz` and `/readyz` follow the Kubernetes convention rather than
//! `/health`. Two endpoints, because they answer different questions and a
//! deployment behaves differently on each: *liveness* failing means restart the
//! process, *readiness* failing means stop sending it traffic but leave it
//! alone. Collapsing them into one endpoint means a slow start-up gets
//! interpreted as a crash loop.

pub mod health;
pub mod predict;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

/// Every path this service serves, in the order the router registers them.
///
/// Duplicating the list looks redundant and is not: `docs/01-srs.md` and the
/// OpenAPI description both enumerate the API surface, and a hand-maintained
/// list in a document drifts silently. This constant is the single place the
/// route table is written down, and the test below is what stops [`build`] and
/// this array from disagreeing.
pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/healthz"),
    ("GET", "/readyz"),
    ("GET", "/version"),
    ("POST", "/predict"),
];

/// Assemble the router.
///
/// Middleware is applied by the caller (`main`), not here, so tests can exercise
/// handlers without a 30-second timeout layer or a CORS policy in the way.
pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::liveness))
        .route("/readyz", get(health::readiness))
        .route("/version", get(health::version))
        // Increment 2. Registered now, returning a typed 503 from a stub, so the
        // route table in the report is the real one and a client written against
        // it gets a documented error rather than a 404 that looks like a typo.
        .route("/predict", post(predict::predict_one))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Liveness and readiness are separate endpoints on purpose -- see the module
    /// docs. The test states the convention so a future "simplification" into one
    /// route has to argue with it.
    #[test]
    fn liveness_and_readiness_are_separate_routes() {
        let paths: Vec<&str> = ROUTES.iter().map(|(_, p)| *p).collect();
        assert!(paths.contains(&"/healthz"), "liveness: restart me");
        assert!(
            paths.contains(&"/readyz"),
            "readiness: stop sending me traffic"
        );
    }

    /// No duplicates, and every path is absolute. Both mistakes produce a router
    /// that builds and then behaves oddly at run time.
    #[test]
    fn the_route_table_is_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for (method, path) in ROUTES {
            assert!(path.starts_with('/'), "{path} must be absolute");
            assert!(
                seen.insert((method, path)),
                "duplicate route: {method} {path}"
            );
        }
    }
}
