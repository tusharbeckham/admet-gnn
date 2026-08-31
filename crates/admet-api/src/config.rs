//! Layered configuration.
//!
//! Manual chapter 26.3, Listing 26.3. Three sources, lowest precedence first:
//!
//! 1. `config/default.toml` -- committed, and the documentation of what is
//!    configurable. A setting that exists only as an environment variable is a
//!    setting nobody knows about.
//! 2. `config/{profile}.toml` -- optional per-environment overrides.
//! 3. `ADMET_*` environment variables -- deployment-time values and every
//!    secret. `ADMET_SERVER__PORT=9000` overrides `[server] port`; the double
//!    underscore is the nesting separator.
//!
//! # Why secrets only ever come from the environment
//!
//! `DATABASE_URL` contains a password. A committed file containing a password is
//! a leaked password the moment the repository is cloned, and `git rm` does not
//! remove it from history. So `config/default.toml` carries a placeholder that
//! obviously will not work, and the real value arrives from the environment --
//! which also means the failure mode of forgetting it is a clear start-up error
//! rather than a connection to something unintended.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::Deserialize;

/// The whole configuration tree.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Listener and limits.
    pub server: ServerConfig,
    /// Connection string and pool size.
    pub database: DatabaseConfig,
    /// Model artefact and threading.
    pub model: ModelConfig,
    /// Cache sizing.
    pub cache: CacheConfig,
}

/// HTTP listener settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Bind address. Defaults to `127.0.0.1`, **not** `0.0.0.0`.
    ///
    /// This is a security default, not an oversight. Until authentication lands
    /// in Increment 3 the service has no access control at all, and a
    /// development binary listening on every interface is reachable from the
    /// local network. Binding to loopback means exposing it is a deliberate
    /// one-line change in the environment rather than the default state.
    ///
    /// In the container, this becomes `0.0.0.0` -- correct there, because the
    /// container's network namespace is the isolation boundary and Caddy
    /// terminates TLS in front.
    pub host: IpAddr,
    /// Listener port.
    pub port: u16,
    /// Request body cap in bytes. TR-06: 20 MB, which is roughly a
    /// 200,000-row SMILES CSV. Unbounded bodies are a memory-exhaustion vector,
    /// and "the client would not do that" is not a control.
    pub max_body_bytes: usize,
    /// Per-request timeout in seconds. TR-06 again. A request that cannot finish
    /// in 30 s is holding a connection and a worker for no benefit.
    pub timeout_secs: u64,
    /// Origins allowed by CORS. Explicit list, never a wildcard: the API will
    /// carry a session cookie from Increment 3, and `Access-Control-Allow-Origin:
    /// *` cannot be combined with credentials -- browsers refuse it, which is the
    /// browser protecting you from a real vulnerability.
    pub cors_origins: Vec<String>,
}

impl ServerConfig {
    /// The socket to bind.
    pub fn addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

/// Persistence settings.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// `postgres://user:password@host:port/database`.
    ///
    /// Supplied by `ADMET_DATABASE__URL` in every real environment. The value in
    /// `config/default.toml` is a placeholder that fails loudly.
    pub url: String,
    /// Pool ceiling. See [`admet_db::DEFAULT_MAX_CONNECTIONS`] for why this is
    /// small on purpose.
    pub max_connections: u32,
    /// Whether to apply migrations at start-up. True for one instance, false for
    /// several -- concurrent instances race on the migration lock and the losers
    /// time out during a deployment.
    pub migrate_on_start: bool,
}

/// Model artefact settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    /// Path to the `.onnx` file.
    pub path: String,
    /// ONNX Runtime intra-op threads.
    ///
    /// One, not "all cores". The server already has request-level parallelism, so
    /// intra-op threads compete with it: two threads per request across sixteen
    /// concurrent requests is thirty-two threads fighting over eight cores, and
    /// throughput falls while every individual latency rises. Measure before
    /// changing this -- `just bench` exists for exactly that argument.
    pub intra_threads: usize,
    /// Micro-batch ceiling. 64 is the knee of the throughput curve in the
    /// criterion sweep, and the sweep is the justification rather than the
    /// number being assumed.
    pub max_batch: usize,
    /// Expected feature-schema version. Checked against
    /// `admet_core::features::SCHEMA_VERSION` at start-up; a mismatch means the
    /// featuriser and the model disagree, so the service **refuses to start**
    /// rather than warning. A warning here produces confidently wrong numbers,
    /// which is worse than an outage because nobody notices.
    pub feature_schema_version: u32,
}

/// In-process cache settings.
#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    /// Total entries across all shards. 50,000 at roughly 200 bytes each is
    /// about 10 MB -- cheap enough that the only reason to tune it is measurement.
    pub capacity: usize,
    /// Shard count. 16 keeps lock contention low without making each shard's LRU
    /// list so short that the eviction policy stops approximating global LRU.
    pub shards: usize,
}

impl Settings {
    /// Load from files plus the environment.
    ///
    /// `profile` selects the optional overlay file, typically from `ADMET_PROFILE`
    /// (`local`, `docker`, `ci`).
    ///
    /// # Errors
    /// [`config::ConfigError`] if a file is malformed or a required key is
    /// missing. Missing keys failing here -- at start-up, by name -- is the whole
    /// point of not using `Option` everywhere with silent defaults.
    pub fn load(profile: &str) -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name(&format!("config/{profile}")).required(false))
            .add_source(
                config::Environment::with_prefix("ADMET")
                    .separator("__")
                    .list_separator(","),
            )
            .build()?
            .try_deserialize()
    }

    /// Hard-coded defaults, for tests and for `--help` output.
    ///
    /// Not a `Default` impl: `Default` invites accidental use in production
    /// paths, and this deliberately points at a database that does not exist so
    /// that using it by mistake fails immediately.
    pub fn for_tests() -> Self {
        Self {
            server: ServerConfig {
                host: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 8080,
                max_body_bytes: 20 * 1024 * 1024,
                timeout_secs: 30,
                cors_origins: vec!["http://localhost:5173".to_owned()],
            },
            database: DatabaseConfig {
                url: "postgres://admet:changeme@localhost:5433/admet_test".to_owned(),
                max_connections: 5,
                migrate_on_start: true,
            },
            model: ModelConfig {
                path: "models/model.onnx".to_owned(),
                intra_threads: 1,
                max_batch: 64,
                feature_schema_version: 1,
            },
            cache: CacheConfig {
                capacity: 1_000,
                shards: 4,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two limits that are security controls rather than tuning knobs.
    #[test]
    fn body_and_timeout_limits_are_present_and_bounded() {
        let s = Settings::for_tests();
        assert_eq!(s.server.max_body_bytes, 20 * 1024 * 1024, "TR-06");
        assert!(
            s.server.timeout_secs > 0,
            "an unbounded timeout is not a timeout"
        );
        assert!(s.server.timeout_secs <= 60);
    }

    /// Loopback by default. If this test ever fails because someone changed the
    /// default to `0.0.0.0`, that is a deployment decision that needs to be made
    /// on purpose and written down -- not inherited from a test fixture.
    #[test]
    fn the_default_bind_address_is_loopback() {
        let s = Settings::for_tests();
        assert!(
            s.server.host.is_loopback(),
            "no auth exists yet; do not listen publicly"
        );
    }

    /// A wildcard origin cannot be combined with credentials, so a wildcard here
    /// would break the browser session in Increment 3 -- discovered as a CORS
    /// error with a message that explains nothing.
    #[test]
    fn cors_origins_are_explicit() {
        let s = Settings::for_tests();
        assert!(!s.server.cors_origins.is_empty());
        assert!(!s.server.cors_origins.iter().any(|o| o == "*"));
    }
}
