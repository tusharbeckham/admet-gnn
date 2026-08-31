//! ADMETriage HTTP service, as a library.
//!
//! Manual chapter 19. The binary in `main.rs` is thin wiring on top of this;
//! everything testable lives here.
//!
//! # Why a library *and* a binary
//!
//! Two reasons, and the second is the one that matters.
//!
//! 1. **Integration tests can build the router without binding a socket.**
//!    `tower::ServiceExt::oneshot` drives [`routes::build`] directly, so the API
//!    tests need no free port, no `sleep`, and no cleanup -- which is why they run
//!    in milliseconds and are worth running on every save.
//! 2. **`dead_code` behaves usefully.** In a binary crate every unreachable item
//!    is a warning, so a scaffold that defines error variants before the handlers
//!    that raise them cannot compile under `-D warnings`. In a library those items
//!    are public API and the lint stays quiet, which means the payload contract
//!    can be written down before the code that fills it in -- exactly what this
//!    scaffold is for.
//!
//! ```text
//! src/lib.rs     config, errors, state, routes   <- tested
//! src/main.rs    tracing, listener, middleware   <- wiring only
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod config;
pub mod error;
pub mod routes;
pub mod state;

pub use config::Settings;
pub use error::{ApiError, Problem};
pub use state::AppState;
