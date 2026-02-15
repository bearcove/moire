//! Diagnostic wrappers for tokio channels, semaphores, and `OnceCell`.
//!
//! When the `diagnostics` feature is enabled, wraps tokio sync primitives
//! to track message counts, channel state, semaphore contention, and OnceCell
//! initialization timing. When disabled, all wrappers are zero-cost.

#[cfg(feature = "diagnostics")]
pub(crate) mod channels;
#[cfg(feature = "diagnostics")]
pub(crate) mod oncecell;
#[cfg(feature = "diagnostics")]
pub(crate) mod semaphore;

#[cfg(feature = "diagnostics")]
mod enabled;
#[cfg(not(feature = "diagnostics"))]
mod disabled;

#[cfg(feature = "diagnostics")]
pub(crate) use enabled::*;
#[cfg(not(feature = "diagnostics"))]
pub(crate) use disabled::*;
