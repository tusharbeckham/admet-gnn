//! RFC 9457 problem details: one error format for the whole API.
//!
//! Manual chapter 19.4. Every failure response is `application/problem+json`
//! with the same five fields, so the web client has exactly one error shape to
//! handle rather than one per endpoint.
//!
//! ```json
//! {
//!   "type": "https://admetriage.dev/problems/invalid-smiles",
//!   "title": "The submitted SMILES could not be parsed",
//!   "status": 400,
//!   "detail": "unexpected character 'Q' at byte 4",
//!   "instance": "/predict",
//!   "position": 4
//! }
//! ```
//!
//! # The field that makes this worth doing
//!
//! `position`. A chemist who typed one wrong character gets told *which*
//! character, and the UI can render a caret under it. "Invalid SMILES" is a dead
//! end; "unexpected character at byte 4" is a fix. RFC 9457 explicitly permits
//! extension members, so carrying it is standard-conformant rather than a
//! private hack.
//!
//! # And the discipline that makes it safe
//!
//! `detail` is written for the user and must never carry internal state -- no SQL
//! text, no file paths, no connection strings. A database error becomes "a
//! required service is unavailable" in the response and the full
//! [`admet_db::DbError`] in the log, correlated by `instance` and the request-id
//! header. Leaking a schema through an error message is a real disclosure, and
//! error paths are where it usually happens because nobody reviews them.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Base URI for problem types. Dereferenceable by convention -- the RFC
/// encourages the `type` URI to resolve to human-readable documentation.
pub const PROBLEM_BASE: &str = "https://admetriage.dev/problems";

/// A problem-details payload.
#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    /// Stable type URI. This is the field clients branch on -- `status` alone is
    /// too coarse, since three different 400s need three different messages in
    /// the UI.
    #[serde(rename = "type")]
    pub type_uri: String,
    /// Short, human-readable summary. Does not vary with the occurrence.
    pub title: &'static str,
    /// HTTP status, duplicated in the body on purpose: the RFC requires it, and
    /// a logged payload is then self-contained.
    pub status: u16,
    /// What went wrong *this time*. User-facing. Never internal state.
    pub detail: String,
    /// The path that produced it.
    pub instance: String,
    /// Byte offset into the submitted SMILES, when the error has one. Skipped
    /// entirely rather than serialised as `null`, because a `null` position
    /// invites the client to render a caret at index 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
}

impl Problem {
    /// Build a problem for `slug` at `instance`.
    pub fn new(
        slug: &str,
        title: &'static str,
        status: StatusCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            type_uri: format!("{PROBLEM_BASE}/{slug}"),
            title,
            status: status.as_u16(),
            detail: detail.into(),
            instance: String::new(),
            position: None,
        }
    }

    /// Attach the request path.
    pub fn at(mut self, instance: impl Into<String>) -> Self {
        self.instance = instance.into();
        self
    }

    /// Attach a byte offset.
    pub fn with_position(mut self, position: usize) -> Self {
        self.position = Some(position);
        self
    }
}

/// The API's error type. Handlers return `Result<T, ApiError>`.
#[derive(Debug)]
pub enum ApiError {
    /// The SMILES did not parse. Carries the byte offset when one is meaningful.
    InvalidSmiles {
        /// Message from `admet_core::smiles::SmilesError`, already user-facing.
        detail: String,
        /// Byte offset, if the underlying error had one.
        position: Option<usize>,
    },
    /// Above the 128-heavy-atom cap. A distinct type from a parse failure
    /// because the user's next action differs: nothing about the string is wrong,
    /// the molecule is simply outside what this model can represent.
    MoleculeTooLarge {
        /// Heavy-atom count found.
        found: usize,
        /// The cap.
        limit: usize,
    },
    /// The resource does not exist.
    NotFound {
        /// What was looked for, for the message.
        what: String,
    },
    /// Malformed request body or parameters.
    BadRequest {
        /// What was wrong with it.
        detail: String,
    },
    /// Body exceeded the configured cap.
    PayloadTooLarge {
        /// Configured limit in bytes.
        limit: usize,
    },
    /// Persistence failed. Logged in full, reported vaguely.
    Database(admet_db::DbError),
    /// Inference failed. Logged in full, reported vaguely.
    Inference(String),
    /// Anything unclassified. Also logged in full and reported vaguely -- a 500
    /// that explains itself to the client is a 500 that explains the internals to
    /// an attacker.
    Internal(String),
}

impl ApiError {
    /// Map to a status and a problem body.
    ///
    /// The two 5xx arms deliberately discard their detail. It is logged by
    /// [`IntoResponse`] before this runs, so nothing is lost -- it just stops at
    /// the process boundary.
    fn problem(&self) -> (StatusCode, Problem) {
        match self {
            Self::InvalidSmiles { detail, position } => {
                let mut p = Problem::new(
                    "invalid-smiles",
                    "The submitted SMILES could not be parsed",
                    StatusCode::BAD_REQUEST,
                    detail.clone(),
                );
                if let Some(pos) = position {
                    p = p.with_position(*pos);
                }
                (StatusCode::BAD_REQUEST, p)
            }
            Self::MoleculeTooLarge { found, limit } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Problem::new(
                    "molecule-too-large",
                    "The molecule exceeds the supported size",
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "{found} heavy atoms; this model supports at most {limit}. \
                         The molecule is not truncated, because a truncated molecule \
                         is a different molecule."
                    ),
                ),
            ),
            Self::NotFound { what } => (
                StatusCode::NOT_FOUND,
                Problem::new(
                    "not-found",
                    "Not found",
                    StatusCode::NOT_FOUND,
                    what.clone(),
                ),
            ),
            Self::BadRequest { detail } => (
                StatusCode::BAD_REQUEST,
                Problem::new(
                    "bad-request",
                    "Malformed request",
                    StatusCode::BAD_REQUEST,
                    detail.clone(),
                ),
            ),
            Self::PayloadTooLarge { limit } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Problem::new(
                    "payload-too-large",
                    "Request body too large",
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("the maximum accepted body is {limit} bytes"),
                ),
            ),
            Self::Database(_) | Self::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Problem::new(
                    "internal",
                    "Internal error",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the request could not be completed; the failure has been logged",
                ),
            ),
            Self::Inference(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Problem::new(
                    "inference-unavailable",
                    "The prediction model is unavailable",
                    StatusCode::SERVICE_UNAVAILABLE,
                    "the model could not score this request; retrying shortly may succeed",
                ),
            ),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSmiles { detail, .. } => write!(f, "invalid smiles: {detail}"),
            Self::MoleculeTooLarge { found, limit } => write!(f, "too large: {found} > {limit}"),
            Self::NotFound { what } => write!(f, "not found: {what}"),
            Self::BadRequest { detail } => write!(f, "bad request: {detail}"),
            Self::PayloadTooLarge { limit } => write!(f, "payload over {limit} bytes"),
            Self::Database(e) => write!(f, "database: {e}"),
            Self::Inference(e) => write!(f, "inference: {e}"),
            Self::Internal(e) => write!(f, "internal: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<admet_db::DbError> for ApiError {
    /// Translates at the boundary. `NotFound` keeps its meaning and becomes a
    /// 404; everything else collapses to a 500, because the client cannot act on
    /// a constraint violation and should not be told about one.
    fn from(e: admet_db::DbError) -> Self {
        match e {
            admet_db::DbError::NotFound { entity, key } => Self::NotFound {
                what: format!("{entity} {key}"),
            },
            other => Self::Database(other),
        }
    }
}

impl IntoResponse for ApiError {
    /// Logs first, then responds.
    ///
    /// Order matters: the 5xx arms discard their detail when building the body,
    /// so if the log line came second the information would already be gone.
    fn into_response(self) -> Response {
        let (status, problem) = self.problem();
        if status.is_server_error() {
            tracing::error!(error = %self, status = status.as_u16(), "request failed");
        } else {
            tracing::debug!(error = %self, status = status.as_u16(), "request rejected");
        }
        let mut response = (status, Json(problem)).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The security property, asserted rather than trusted: no 5xx body may
    /// contain anything from the underlying error. This is the test that fails if
    /// someone helpfully adds `format!("{e}")` to the detail during debugging and
    /// forgets to take it out.
    #[test]
    fn server_errors_never_leak_internal_detail() {
        let leaky = "relation \"molecules\" does not exist at /home/user/src/repo.rs:42";
        for e in [
            ApiError::Internal(leaky.to_owned()),
            ApiError::Database(admet_db::DbError::Corrupt(leaky.to_owned())),
            ApiError::Inference(leaky.to_owned()),
        ] {
            let (status, problem) = e.problem();
            assert!(status.is_server_error());
            let body = serde_json::to_string(&problem).unwrap();
            assert!(!body.contains("molecules"), "schema leaked: {body}");
            assert!(!body.contains("/home/user"), "path leaked: {body}");
            assert!(!body.contains(".rs:42"), "source location leaked: {body}");
        }
    }

    /// Client errors are the opposite case: the detail is the entire value, and
    /// the position is what lets the UI point at the mistake.
    #[test]
    fn client_errors_keep_their_detail_and_position() {
        let e = ApiError::InvalidSmiles {
            detail: "unexpected character 'Q' at byte 4".to_owned(),
            position: Some(4),
        };
        let (status, problem) = e.problem();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(problem.position, Some(4));
        assert!(problem.detail.contains("byte 4"));
        assert_eq!(
            problem.type_uri,
            "https://admetriage.dev/problems/invalid-smiles"
        );
    }

    /// A `null` position would invite a caret at index 0, which points at the
    /// wrong character with total confidence.
    #[test]
    fn absent_position_is_omitted_not_null() {
        let p = Problem::new(
            "not-found",
            "Not found",
            StatusCode::NOT_FOUND,
            "no such batch",
        );
        let body = serde_json::to_string(&p).unwrap();
        assert!(
            !body.contains("position"),
            "position must be absent: {body}"
        );
        assert!(
            body.contains("\"type\":"),
            "the RFC field is `type`, not `type_uri`"
        );
    }

    /// An oversized molecule is not a malformed one, and the distinction changes
    /// what the user should do next.
    #[test]
    fn oversized_molecules_are_422_not_400() {
        let (status, problem) = (ApiError::MoleculeTooLarge {
            found: 200,
            limit: 128,
        })
        .problem();
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(problem.detail.contains("128"));
        assert!(
            problem.detail.contains("truncated"),
            "explain why we refuse rather than truncate"
        );
    }
}
