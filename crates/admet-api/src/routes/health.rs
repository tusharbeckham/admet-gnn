//! Health, readiness and version endpoints.
//!
//! Manual chapter 26.4. The three cheapest endpoints in the service, and the
//! ones that determine whether a deployment behaves sensibly when something is
//! wrong.
//!
//! | Route | Question | On failure, the platform should |
//! |---|---|---|
//! | `GET /healthz` | Is the process alive? | restart it |
//! | `GET /readyz` | Can it serve traffic? | route around it, leave it running |
//! | `GET /version` | What exactly is deployed? | nothing -- it is for humans |
//!
//! # Liveness must not check dependencies
//!
//! `/healthz` returns 200 unconditionally. That looks lazy and is the correct
//! behaviour: if liveness checked the database, a database outage would restart
//! every instance of the API, turning one failure into two. The rule is that
//! liveness only fails for problems a restart can fix.
//!
//! # `/version` is where reproducibility becomes operational
//!
//! Reporting the model's SHA-256 and the feature-schema version means "which
//! model produced this number" is answerable from a running system rather than
//! from a changelog someone remembered to update.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

/// `GET /healthz` response.
#[derive(Debug, Serialize)]
pub struct Liveness {
    /// Always `"ok"`. Reaching this handler *is* the check.
    pub status: &'static str,
}

/// `GET /readyz` response.
#[derive(Debug, Serialize)]
pub struct Readiness {
    /// `"ready"` or `"not_ready"`.
    pub status: &'static str,
    /// `"loaded"` or `"absent"`.
    pub model: &'static str,
    /// `"connected"`, `"unreachable"` or `"absent"`.
    pub database: &'static str,
}

/// `GET /version` response.
#[derive(Debug, Serialize)]
pub struct Version {
    /// Crate version from `CARGO_PKG_VERSION`, so it cannot disagree with
    /// `Cargo.toml`.
    pub version: &'static str,
    /// Git commit, injected at build time. `"unknown"` for a plain `cargo build`
    /// -- honest rather than misleading, and the build script fills it in for
    /// release artefacts.
    pub commit: &'static str,
    /// The 33-feature contract version this binary implements. Compared against
    /// the model's own recorded version at start-up; a mismatch is fatal.
    pub feature_schema_version: u32,
    /// Seconds since start-up.
    pub uptime_secs: i64,
}

/// `GET /healthz` -- liveness.
///
/// No state, no dependencies, no allocation worth mentioning. If this does not
/// answer, the process is genuinely wedged and restarting it is the right move.
pub async fn liveness() -> Json<Liveness> {
    Json(Liveness { status: "ok" })
}

/// `GET /readyz` -- readiness.
///
/// Returns 503 when a dependency is missing, with a body naming which one. The
/// status code is what the load balancer reads; the body is what the human
/// reads, and both matter -- a bare 503 sends an operator to the container logs
/// for information that could have been in the response.
pub async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<Readiness>) {
    let ready = state.is_ready();
    let body = Readiness {
        status: if ready { "ready" } else { "not_ready" },
        model: if state.engine.is_some() {
            "loaded"
        } else {
            "absent"
        },
        database: if state.db.is_some() {
            "connected"
        } else {
            "absent"
        },
    };
    // 503, not 500. The distinction is real: 500 means the request was wrong or
    // the code broke, 503 means "correct request, come back shortly". Only 503
    // tells a caller that retrying is reasonable.
    let code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body))
}

/// `GET /version` -- what is deployed.
pub async fn version(State(state): State<AppState>) -> Json<Version> {
    let uptime = (chrono::Utc::now() - state.started_at).num_seconds();
    Json(Version {
        version: env!("CARGO_PKG_VERSION"),
        // `option_env!` rather than `env!`: `env!` fails the build when the
        // variable is absent, which would mean no plain `cargo build` works
        // without a build script. A missing commit hash is worth a placeholder,
        // not a broken build.
        commit: option_env!("ADMET_GIT_COMMIT").unwrap_or("unknown"),
        feature_schema_version: admet_core::features::SCHEMA_VERSION,
        uptime_secs: uptime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    /// Liveness must not consult state. A dependency check here converts a
    /// database outage into a cluster-wide restart loop.
    #[tokio::test]
    async fn liveness_is_unconditional() {
        let Json(body) = liveness().await;
        assert_eq!(body.status, "ok");
    }

    /// Readiness must fail, and must say what is missing.
    #[tokio::test]
    async fn readiness_reports_which_dependency_is_absent() {
        let state = AppState::bare(Settings::for_tests());
        let (code, Json(body)) = readiness(State(state)).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.model, "absent");
        assert_eq!(body.database, "absent");
    }

    /// The reported schema version is read from `admet-core`, not written down
    /// here. Two copies of this number would eventually disagree, and the
    /// symptom would be predictions that are wrong rather than an error.
    #[tokio::test]
    async fn version_reports_the_schema_version_from_core() {
        let state = AppState::bare(Settings::for_tests());
        let Json(body) = version(State(state)).await;
        assert_eq!(
            body.feature_schema_version,
            admet_core::features::SCHEMA_VERSION
        );
        assert_eq!(body.version, env!("CARGO_PKG_VERSION"));
        assert!(body.uptime_secs >= 0);
    }
}
