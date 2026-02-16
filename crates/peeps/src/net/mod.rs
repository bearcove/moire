//! Diagnostic wrappers for network I/O readiness waits.
//!
//! When `diagnostics` is enabled, wraps connect, accept, readable, and writable
//! operations to register graph nodes with transport kind, endpoint addresses,
//! and timing. When disabled, all wrappers are zero-cost pass-throughs.

#[cfg(feature = "diagnostics")]
mod enabled;
#[cfg(not(feature = "diagnostics"))]
mod disabled;

#[cfg(feature = "diagnostics")]
pub use enabled::*;
#[cfg(not(feature = "diagnostics"))]
pub use disabled::*;
