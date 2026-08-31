//! HTTP-level tests for the assembled router.
//!
//! # Why these exist alongside the handler unit tests
//!
//! `routes::health`'s unit tests call `liveness()` and `readiness()` directly.
//! Those tests are worth having — they pin the liveness-must-not-check-dependencies
//! rule — but they cannot fail for a wiring mistake. Wire `/healthz` to the
//! readiness handler, register it as `POST`, or typo it as `/health`, and every
//! one of them still passes while the deployed service is broken in the specific
//! way that makes a platform restart-loop a healthy process.
//!
//! These tests drive the `Router` that `routes::build` returns, through
//! `tower::ServiceExt::oneshot`, so path, method, status code and response body
//! are all exercised. No socket is bound and no port is chosen, which means they
//! run in CI and cannot fail because something else already holds 8080.
//!
//! This file exists because the equivalent check was first performed by hand —
//! start the binary, `curl /healthz`, read the 200. A verification that only
//! happens when somebody remembers to do it is not a verification.

use admet_api::{routes, AppState, Settings};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt; // for `oneshot`

/// A router with no model and no database, which is the honest state of the
/// service at the scaffold tag.
fn app() -> axum::Router {
    routes::build(AppState::bare(Settings::for_tests()))
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("body is readable and within the limit");
    String::from_utf8(bytes.to_vec()).expect("body is UTF-8")
}

#[tokio::test]
async fn get_healthz_returns_200_ok() {
    let response = app()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_string(response).await, r#"{"status":"ok"}"#);
}

/// Liveness must stay 200 with **no** dependencies present. This is the whole
/// point of the endpoint: if it consulted the database, a database outage would
/// restart every instance and turn one failure into two.
#[tokio::test]
async fn healthz_is_200_even_with_no_model_and_no_database() {
    let state = AppState::bare(Settings::for_tests());
    assert!(state.engine.is_none(), "no model, deliberately");
    assert!(state.db.is_none(), "no database, deliberately");

    let response = routes::build(state)
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);
}

/// Readiness must disagree with liveness when dependencies are missing. If both
/// returned 200 the pair would carry no information, and the reason for having
/// two endpoints would be gone.
#[tokio::test]
async fn readyz_returns_503_and_names_the_missing_dependencies() {
    let response = app()
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = body_string(response).await;
    assert!(body.contains(r#""status":"not_ready""#), "body was {body}");
    assert!(body.contains(r#""model":"absent""#), "body was {body}");
    assert!(body.contains(r#""database":"absent""#), "body was {body}");
}

#[tokio::test]
async fn get_version_reports_the_crate_and_schema_version() {
    let response = app()
        .oneshot(Request::get("/version").body(Body::empty()).unwrap())
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response).await;
    assert!(
        body.contains(env!("CARGO_PKG_VERSION")),
        "version missing from {body}"
    );
    //  Read from admet-core rather than written here. Two copies of this number
    //  would eventually disagree, and the symptom would be wrong predictions
    //  rather than an error.
    let schema = admet_core::features::SCHEMA_VERSION;
    assert!(
        body.contains(&format!(r#""feature_schema_version":{schema}"#)),
        "schema version {schema} missing from {body}"
    );
}

/// `/predict` is registered now and returns a typed 503 from a stub, so a client
/// written against the documented route table gets a documented error rather than
/// a 404 that looks like a typo. Asserting 503-not-404 is what keeps that promise
/// honest before Increment 2 fills the handler in.
#[tokio::test]
async fn post_predict_is_registered_and_returns_503_not_404() {
    let response = app()
        .oneshot(
            Request::post("/predict")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"smiles":"c1ccccc1"}"#))
                .unwrap(),
        )
        .await
        .expect("router responds");

    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "/predict must be registered even while it is a stub"
    );
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// A GET to a POST-only route must be 405, not 404. The distinction matters to a
/// client author: 404 says "you invented this endpoint", 405 says "right
/// endpoint, wrong verb".
#[tokio::test]
async fn get_predict_is_405_not_404() {
    let response = app()
        .oneshot(Request::get("/predict").body(Body::empty()).unwrap())
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn unknown_route_is_404() {
    let response = app()
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .expect("router responds");

    //  `/health` specifically: it is the name everyone reaches for, and if a typo
    //  ever silently answered here the real `/healthz` contract would rot.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
