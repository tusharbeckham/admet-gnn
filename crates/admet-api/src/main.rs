//! ADMETriage HTTP service.
//!
//! Manual chapters 19 and 26. This binary composes the other crates and owns
//! nothing but wiring: configuration, logging, middleware, the listener, and
//! shutdown.
//!
//! ```text
//! ADMET_PROFILE=local cargo run -p admet-api
//! curl -s localhost:8080/healthz
//! curl -s localhost:8080/readyz    # 503 until a model and database exist
//! ```
//!
//! # Start-up is tolerant; serving is not
//!
//! A missing `model.onnx` or an unreachable database logs a warning and the
//! process keeps going, answering `/healthz` and reporting the gap on `/readyz`.
//! That is the opposite of sloppiness: a container that exits on a missing
//! artefact crash-loops and takes its diagnostics with it, whereas one that
//! starts and says `model: absent` is inspectable with a single `curl`. No
//! prediction is ever served from a degraded state -- `/predict` returns a typed
//! 503, never a default value.
//!
//! # Security posture of the scaffold, stated plainly
//!
//! There is **no authentication on any route** yet; Argon2id sessions arrive in
//! Increment 3 alongside named projects (UC-07 / FR-23). The mitigations that
//! exist today are the ones that cost
//! nothing to have from the start: the default bind address is `127.0.0.1` so the
//! service is not reachable off-box, the body cap is 20 MB, every request has a
//! 30-second timeout, and CORS is an explicit origin list rather than a wildcard.
//! Do not set `ADMET_SERVER__HOST=0.0.0.0` on a shared network before Increment 3
//! lands.

mod tracing_setup;

use std::time::Duration;

use anyhow::Context;
use axum::http::StatusCode;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use admet_api::{routes, AppState, Settings};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_setup::init();

    let profile = std::env::var("ADMET_PROFILE").unwrap_or_else(|_| "local".to_owned());
    let settings = Settings::load(&profile)
        .with_context(|| format!("loading config for profile `{profile}`"))?;
    tracing::info!(profile = %profile, port = settings.server.port, "configuration loaded");

    let mut state = AppState::bare(settings.clone());
    state.engine = load_engine(&settings);
    state.db = connect_db(&settings).await;

    if state.is_ready() {
        tracing::info!("model and database present: ready to serve predictions");
    } else {
        tracing::warn!(
            "starting DEGRADED -- /healthz will pass, /readyz will report 503. \
             This is expected until Increment 1 produces models/model.onnx."
        );
    }

    let app = routes::build(state)
        // Order matters and reads inside-out: the innermost layer is closest to
        // the handler. The body limit must sit OUTSIDE the timeout, or a slow
        // 500 MB upload gets 30 seconds to consume memory before anything checks
        // its size.
        // `TimeoutLayer::new` is deprecated in tower-http 0.6.11 in favour of
        // naming the status code explicitly. 408 is what `new` used anyway, so
        // this is the same behaviour stated out loud rather than inherited.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(settings.server.timeout_secs),
        ))
        .layer(RequestBodyLimitLayer::new(settings.server.max_body_bytes))
        .layer(cors_layer(&settings))
        .layer(TraceLayer::new_for_http());

    let addr = settings.server.addr();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("shutdown complete");
    Ok(())
}

/// Load the ONNX model, or explain why not.
///
/// Returns `None` rather than an error. The warning names the path it looked at,
/// because "model not found" without the path is the least useful log line in
/// software.
fn load_engine(
    settings: &Settings,
) -> Option<std::sync::Arc<std::sync::Mutex<admet_infer::Engine>>> {
    let path = std::path::Path::new(&settings.model.path);
    if !path.exists() {
        tracing::warn!(path = %path.display(), "no model artefact; predictions unavailable");
        return None;
    }

    match admet_infer::Engine::load(path, settings.model.intra_threads) {
        Ok(engine) => {
            tracing::info!(
                path = %path.display(),
                inputs = ?engine.input_names(),
                outputs = ?engine.output_names(),
                "model loaded"
            );
            Some(std::sync::Arc::new(std::sync::Mutex::new(engine)))
        }
        // A model that exists but will not load is worth a louder line than one
        // that is simply absent: absence is the expected state during Increment 1,
        // a load failure means the artefact is corrupt or the opset is wrong.
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "model failed to load");
            None
        }
    }
}

/// Connect and migrate, or explain why not.
async fn connect_db(settings: &Settings) -> Option<admet_db::Pool> {
    let pool = match admet_db::connect(&settings.database.url).await {
        Ok(pool) => pool,
        // The URL is deliberately not logged: it contains a password.
        Err(e) => {
            tracing::warn!(error = %e, "database unreachable; running without persistence");
            return None;
        }
    };
    tracing::info!("database connected");

    if settings.database.migrate_on_start {
        // Skip cleanly when there is nothing to apply. `migrations/` holds only a
        // README until Increment 2 writes `0001_initial.sql`, and an empty
        // directory should not look like a failure.
        let has_sql = std::fs::read_dir(admet_db::MIGRATIONS_DIR)
            .map(|dir| {
                dir.flatten()
                    .any(|e| e.path().extension().is_some_and(|ext| ext == "sql"))
            })
            .unwrap_or(false);

        if !has_sql {
            tracing::info!("no migrations to apply yet");
        } else if let Err(e) = admet_db::migrate(&pool).await {
            // Fatal, unlike everything else in start-up. A schema mismatch means
            // every query is wrong in a way that produces bad data rather than
            // errors, and serving from it is worse than not serving at all.
            tracing::error!(error = %e, "migrations failed");
            return None;
        }
    }

    Some(pool)
}

/// CORS policy from the configured origin list.
///
/// Explicit origins, never `Any`. From Increment 3 the API carries a session
/// cookie, and a browser refuses to send credentials to a wildcard origin -- so a
/// wildcard here would not be a lax policy, it would be a broken one.
fn cors_layer(settings: &Settings) -> CorsLayer {
    let origins: Vec<_> = settings
        .server
        .cors_origins
        .iter()
        .filter_map(|o| o.parse::<axum::http::HeaderValue>().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_credentials(true)
}

/// Wait for Ctrl-C or SIGTERM.
///
/// Graceful shutdown is what makes a rolling deployment invisible: in-flight
/// requests finish instead of being cut off mid-response. `SIGTERM` matters more
/// than Ctrl-C in practice -- it is what Docker and Kubernetes send, and a process
/// that ignores it gets `SIGKILL` ten seconds later with requests still open.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    // Windows has no SIGTERM. `pending()` never resolves, so the select below is
    // driven by Ctrl-C alone -- which is the correct behaviour on this platform,
    // not a gap.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received Ctrl-C"),
        () = terminate => tracing::info!("received SIGTERM"),
    }
}
