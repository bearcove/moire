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

// Re-export public API items for external crates.
#[cfg(not(feature = "diagnostics"))]
pub use disabled::with_top;
#[cfg(feature = "diagnostics")]
pub use enabled::with_top;

// Re-export crate-internal items (push, pop)
#[cfg(not(feature = "diagnostics"))]
pub(crate) use disabled::{pop, push};
#[cfg(feature = "diagnostics")]
pub(crate) use enabled::{pop, push};

// Re-export public API items (ensure) for entrypoint initialization
#[cfg(not(feature = "diagnostics"))]
pub use disabled::ensure;
#[cfg(feature = "diagnostics")]
pub use enabled::ensure;

// Re-export request-scope helpers.
#[cfg(not(feature = "diagnostics"))]
pub use disabled::{capture_top, is_active, scope};
#[cfg(feature = "diagnostics")]
pub use enabled::{capture_top, is_active, scope};
