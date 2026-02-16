//! Diagnostic wrapper for `tokio::process::Command`.
//!
//! When `diagnostics` is enabled, wraps command execution to register
//! graph nodes with program, args, exit code, and timing information.
//! When disabled, all wrappers are zero-cost pass-throughs.

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(feature = "diagnostics")]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub use disabled::{Child, Command};
#[cfg(feature = "diagnostics")]
pub use enabled::{Child, Command};
