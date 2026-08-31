//! Logging initialisation.
//!
//! Manual chapter 26.5. Split out of `main.rs` because it is the one piece of
//! start-up with a real decision in it.

/// Install the global subscriber. Level from `RUST_LOG`.
///
/// Two output formats, because they have different readers: a person tailing a
/// terminal wants aligned columns, and a log aggregator wants fields it can
/// index. One format always disappoints one of them, so
/// `ADMET_LOG_FORMAT=json` picks.
///
/// The fallback filter is not just `info`. `tower_http=debug` is what produces a
/// line per request with method, path, status and duration -- and that line is
/// where the latency evidence for NFR-01 comes from in development, before there
/// is a metrics endpoint to ask instead.
pub fn init() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,admet_api=debug,tower_http=debug"));

    if std::env::var("ADMET_LOG_FORMAT").as_deref() == Ok("json") {
        fmt().with_env_filter(filter).json().init();
    } else {
        fmt().with_env_filter(filter).init();
    }
}
