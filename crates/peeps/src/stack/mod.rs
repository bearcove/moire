//! Task-local async node stack for canonical graph edge emission.
//!
//! Maintains a logical stack of instrumented nodes (futures) per async task.
//! Only the top of the stack is allowed to emit `needs` edges to resources.
//!
//! When `diagnostics` is disabled, all operations compile away to no-ops.

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(feature = "diagnostics")]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub(crate) use disabled::*;
#[cfg(feature = "diagnostics")]
pub(crate) use enabled::*;
