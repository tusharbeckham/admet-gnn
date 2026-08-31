//! Row types: the Rust side of the ER model.
//!
//! Manual chapters 11 and 12. One struct per table, named for the table, with
//! the column comments that explain *why* the column is that type.
//!
//! # The normalisation decision that matters
//!
//! Twelve endpoints could be twelve columns on `predictions`. They are not.
//! They are rows in [`PredictionValue`], because:
//!
//! - adding a thirteenth endpoint becomes an `INSERT` into `endpoints`, not an
//!   `ALTER TABLE` plus a code change in five places;
//! - most endpoints are missing for most molecules during evaluation, and a wide
//!   table stores that as a hundred thousand NULLs;
//! - "show me every compound whose hERG prediction is below 0.3" is one indexed
//!   predicate on a narrow table, and twelve `OR`s on a wide one.
//!
//! The cost is a join and one row per value. That is the normal trade and it
//! lands on the side of 3NF here.
//!
//! # And the one that looks like a mistake but is not
//!
//! Deterministic descriptors (molecular weight, cLogP, TPSA, ring count) live on
//! [`Molecule`], not on [`Prediction`]. They depend *only* on the structure, so
//! putting them on a prediction row would store the same value once per model
//! version and invite two copies to disagree. Predicted values depend on
//! `(molecule, model_version)`; measured ones depend on the molecule alone. The
//! table each lands in follows from what it functionally depends on -- which is
//! the definition of normalisation, applied rather than recited.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unique molecular structure. One row per distinct compound, ever.
///
/// `inchikey` is `CHAR(27) UNIQUE NOT NULL` -- the identity column, per ADR-04.
/// Fixed width means a compact B-tree index and no length surprises at the
/// boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Molecule {
    /// Surrogate key. UUIDv7, not v4: v7 is time-ordered, so inserts append to
    /// the right-hand edge of the index instead of scattering across it. On a
    /// bulk import of 10,000 rows that difference is measurable.
    pub id: Uuid,
    /// The identity key. 27 characters, always. Stored as `String` because sqlx
    /// decodes `CHAR(n)` to `String`; convert to
    /// [`admet_core::canonical::InchiKey`] at the crate boundary so the rest of
    /// the system handles a validated type.
    pub inchikey: String,
    /// Canonical SMILES for display and re-parsing. Derived from `inchikey`'s
    /// molecule but not from the key itself -- an InChIKey is a hash and cannot
    /// be inverted, so this column is not redundant.
    pub canonical_smiles: String,
    /// Heavy-atom count. Denormalised deliberately: it is the filter for the
    /// 128-atom cap and re-parsing every SMILES to count atoms is absurd.
    pub n_heavy_atoms: i32,
    /// Deterministic descriptors as JSONB -- exact computed chemistry, never
    /// predictions. JSONB rather than columns because the set grows (QED, SA
    /// score and the PAINS alerts arrive in Increment 3) and none of it is
    /// queried by predicate.
    pub descriptors: serde_json::Value,
    /// First time this structure was seen.
    pub created_at: DateTime<Utc>,
}

/// One of the twelve ADMET endpoints. A lookup table, not an enum in the
/// database.
///
/// Keeping it as data means the web UI can render endpoint names, units and
/// categories from a query instead of a hard-coded list that drifts out of step
/// with the model. `code` is the stable short key used everywhere else
/// (`"bbb"`, `"herg"`), matching `ENDPOINTS` in `training/data/download_tdc.py`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Endpoint {
    /// Stable short code, primary key. Must match the Python `ENDPOINTS` keys
    /// exactly -- this is a parity surface like any other (TR-03).
    pub code: String,
    /// The TDC dataset name, e.g. `BBB_Martins`. Provenance for the report.
    pub tdc_name: String,
    /// `absorption` | `distribution` | `metabolism` | `excretion` | `toxicity`.
    /// Drives the triage weights in `admet_core::triage::Weights`.
    pub category: String,
    /// `binary` or `regression`. Determines whether `value` is a probability or
    /// a physical quantity, and therefore how the UI must render it.
    pub task: String,
    /// Unit for regression endpoints (`"log cm/s"`, `"hours"`), `None` for
    /// binary ones. A number shown without its unit is not a result.
    pub unit: Option<String>,
    /// Column position in the model's 12-vector output. The ONNX graph returns
    /// an ordered tensor, so something must record the order; recording it here
    /// keeps it beside the endpoint rather than in a constant somewhere.
    pub output_index: i32,
}

/// A trained model artefact.
///
/// Predictions are meaningless without this row. "AUROC 0.83" is a fact about a
/// specific `.onnx` file, and if the file cannot be identified the number cannot
/// be reproduced or defended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelVersion {
    /// Surrogate key.
    pub id: Uuid,
    /// Monotonic version number. Part of the cache key, which is what makes a
    /// model upgrade safe: bump this and every stale cache entry becomes
    /// unreachable rather than wrong.
    pub version: i32,
    /// SHA-256 of the `.onnx` file. The only way to prove the artefact serving
    /// traffic is the artefact that was evaluated.
    pub onnx_sha256: String,
    /// Version of the 33-feature contract this model was trained against. If it
    /// disagrees with `admet_core::features::SCHEMA_VERSION`, the featuriser and
    /// the model are out of step and every prediction is quietly wrong -- so the
    /// service must refuse to start, not warn.
    pub feature_schema_version: i32,
    /// When training finished.
    pub trained_at: DateTime<Utc>,
    /// Held-out metrics as JSON, copied from `results/metrics.json`. Stored so
    /// the UI can show a model card beside a prediction.
    pub metrics: serde_json::Value,
    /// Whether this version serves traffic. Exactly one row should be true;
    /// enforce it with a partial unique index rather than in application code.
    pub is_active: bool,
}

/// One prediction run: a molecule scored by a model version.
///
/// The row exists even when the triage score is withheld, because "we saw this
/// molecule and refused to score it" is a result worth caching and worth
/// counting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Prediction {
    /// Surrogate key.
    pub id: Uuid,
    /// The molecule. `UNIQUE (molecule_id, model_version_id)` -- that pair is
    /// the cache key from ADR-04, expressed as a constraint so a race between
    /// two requests for the same compound cannot produce two rows.
    pub molecule_id: Uuid,
    /// The model that produced it.
    pub model_version_id: Uuid,
    /// Exactly what the user typed, before canonicalisation. Kept so the UI can
    /// echo it back; a chemist who submitted `OCC` should not be shown `CCO` and
    /// left wondering whether the system understood them.
    pub input_smiles: String,
    /// `in_domain` | `low_confidence` | `out_of_domain`. String, matching
    /// `admet_core::fingerprint::DomainStatus::as_str`, so the wire format, the
    /// column and the Rust enum all use one vocabulary.
    pub domain_status: String,
    /// Mean top-5 Tanimoto against the reference set. Stored because the
    /// threshold may be retuned, and a stored similarity can be re-thresholded
    /// while a stored verdict cannot.
    pub domain_similarity: f32,
    /// Weighted geometric mean of per-endpoint desirability, or `None` when
    /// withheld for being out of domain (FR-12). Nullable on purpose: a
    /// withheld score stored as 0.0 would sort as "terrible compound", which is
    /// a different and wrong claim.
    pub triage_score: Option<f32>,
    /// Wall-clock inference time. Populated on every row, which is what turns
    /// the NFR-01 latency claim into a distribution instead of an anecdote.
    pub latency_ms: i32,
    /// When it ran.
    pub created_at: DateTime<Utc>,
}

/// One endpoint value belonging to a [`Prediction`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PredictionValue {
    /// Parent prediction. `PRIMARY KEY (prediction_id, endpoint_code)`: a
    /// natural composite key, so no surrogate id and no possibility of two rows
    /// for the same endpoint of the same prediction.
    pub prediction_id: Uuid,
    /// Which endpoint. FK to [`Endpoint::code`].
    pub endpoint_code: String,
    /// The value. Probability in `[0, 1]` for binary endpoints, physical
    /// quantity in [`Endpoint::unit`] for regression ones -- de-standardised
    /// before storage, because a stored z-score is a number nobody can read.
    pub value: f32,
    /// Per-endpoint desirability in `[0, 1]`, the triage input. Stored so the UI
    /// can show *why* a compound ranked where it did without recomputing the
    /// scoring function in TypeScript -- which would be a second implementation
    /// of a rule, and rules with two implementations diverge.
    pub desirability: f32,
}

/// A batch screening job.
///
/// Long-running and resumable. `completed_rows` is checkpointed every 250 rows
/// so a crash at row 9,000 of 10,000 resumes rather than restarts -- see FR-16.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Batch {
    /// Surrogate key, also the handle the client polls for progress.
    pub id: Uuid,
    /// Owning project.
    pub project_id: Uuid,
    /// Human label, usually the uploaded filename.
    pub name: String,
    /// Lifecycle state. See [`BatchStatus`].
    pub status: String,
    /// Rows in the submitted CSV, known up front.
    pub total_rows: i32,
    /// Rows scored so far. Checkpointed, not incremented per row: 10,000
    /// single-row `UPDATE`s cost more than the inference they track.
    pub completed_rows: i32,
    /// Rows rejected -- unparseable SMILES, over the atom cap. Surfaced to the
    /// user with reasons, never silently dropped.
    pub failed_rows: i32,
    /// Submission time.
    pub created_at: DateTime<Utc>,
    /// Completion time, `None` while running.
    pub finished_at: Option<DateTime<Utc>>,
}

/// Batch lifecycle states.
///
/// A Rust enum with string mapping rather than a PostgreSQL `ENUM` type:
/// PostgreSQL enums cannot have values removed and reordering them requires a
/// rewrite, so a `TEXT` column with a `CHECK` constraint is easier to live with
/// while the state machine is still settling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// Accepted, not started.
    Queued,
    /// Rows being scored.
    Running,
    /// Every row either scored or recorded as failed.
    Completed,
    /// Abandoned. `failed_rows` and the log say why.
    Failed,
    /// Cancelled by the user.
    Cancelled,
}

impl BatchStatus {
    /// The stored string. Stable -- these values are in the database and on the
    /// wire, so changing one is a migration plus an API version, not a rename.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse from the stored string. `None` for anything unrecognised, which the
    /// repository turns into [`crate::DbError::Corrupt`] rather than guessing.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }

    /// Whether the job has stopped moving. Polling clients stop at this point.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A user's workspace: a named collection of molecules and batches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    /// Surrogate key.
    pub id: Uuid,
    /// Owner.
    pub user_id: Uuid,
    /// Display name. `UNIQUE (user_id, name)` -- unique per user, not globally.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// An account.
///
/// # Do not put a password in this struct
///
/// `password_hash` holds an Argon2id PHC string (`$argon2id$v=19$m=19456,...`),
/// which carries its own salt and parameters. There is deliberately no
/// `password` field anywhere in this crate: a plaintext password that never
/// exists as a struct field cannot be logged by a stray `Debug` derive, and
/// accidental credential logging is one of the most common ways real systems
/// leak.
///
/// `Debug` is *not* derived here for the same reason -- see the manual
/// implementation below, which prints the id and email and redacts the hash.
#[derive(Clone, PartialEq, sqlx::FromRow)]
pub struct User {
    /// Surrogate key.
    pub id: Uuid,
    /// Login identity. `UNIQUE`, stored lower-cased so `A@b.com` and `a@b.com`
    /// cannot become two accounts.
    pub email: String,
    /// Argon2id PHC string. Never a plaintext password, never a bare hash
    /// without parameters.
    pub password_hash: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl std::fmt::Debug for User {
    /// Redacts the hash.
    ///
    /// A password hash in a log line is not as bad as a plaintext password, but
    /// it is still credential material handed to whoever can read the logs --
    /// and `tracing::debug!(?user)` is one keystroke away at all times. The
    /// cheapest defence is making the type unable to print it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("email", &self.email)
            .field("password_hash", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These strings are in the database and on the wire. A rename is a
    /// migration, so the test states them literally -- if someone changes the
    /// enum, this fails and asks them whether they meant to.
    #[test]
    fn batch_status_strings_are_stable() {
        for (status, text) in [
            (BatchStatus::Queued, "queued"),
            (BatchStatus::Running, "running"),
            (BatchStatus::Completed, "completed"),
            (BatchStatus::Failed, "failed"),
            (BatchStatus::Cancelled, "cancelled"),
        ] {
            assert_eq!(status.as_str(), text);
            assert_eq!(BatchStatus::parse(text), Some(status));
        }
        assert_eq!(
            BatchStatus::parse("QUEUED"),
            None,
            "parsing is exact, not lenient"
        );
        assert_eq!(BatchStatus::parse(""), None);
    }

    /// A polling client loops until `is_terminal`. If a stopped state ever
    /// returned false, that client would poll forever.
    #[test]
    fn every_stopped_state_is_terminal() {
        assert!(!BatchStatus::Queued.is_terminal());
        assert!(!BatchStatus::Running.is_terminal());
        assert!(BatchStatus::Completed.is_terminal());
        assert!(BatchStatus::Failed.is_terminal());
        assert!(BatchStatus::Cancelled.is_terminal());
    }

    /// The security control from [`User`]'s doc comment, asserted rather than
    /// described. A future `#[derive(Debug)]` would break this test, which is
    /// the whole point.
    #[test]
    fn debug_output_never_contains_the_password_hash() {
        let user = User {
            id: Uuid::nil(),
            email: "chemist@example.org".to_owned(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_owned(),
            created_at: Utc::now(),
        };
        let printed = format!("{user:?}");
        assert!(
            !printed.contains("argon2id"),
            "the hash leaked into Debug: {printed}"
        );
        assert!(printed.contains("<redacted>"));
        assert!(
            printed.contains("chemist@example.org"),
            "email is fine to print"
        );
    }
}
