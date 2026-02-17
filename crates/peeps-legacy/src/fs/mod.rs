//! Diagnostic wrappers for async filesystem operations.
//!
//! When `diagnostics` is enabled, wraps common `tokio::fs` functions to
//! register graph nodes with operation type, path, byte counts, and timing.
//! When disabled, all wrappers are zero-cost pass-throughs.

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(feature = "diagnostics")]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub use disabled::*;
#[cfg(feature = "diagnostics")]
pub use enabled::*;
