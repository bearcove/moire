//! Diagnostic wrappers for locks (both sync and async).
//!
//! When the `diagnostics` feature is enabled, every lock registers
//! itself in the central registry and tracks waiters/holders with
//! stack-based edge emission.
//!
//! When disabled, these are zero-cost wrappers that compile down to
//! plain locks.

#[cfg(feature = "diagnostics")]
mod enabled;
#[cfg(not(feature = "diagnostics"))]
mod disabled;

#[cfg(feature = "diagnostics")]
pub(crate) use enabled::*;
#[cfg(not(feature = "diagnostics"))]
pub(crate) use disabled::*;
