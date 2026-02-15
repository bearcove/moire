//! Unified diagnostics registry for all tracked resource types.
//!
//! Central storage for all live diagnostics objects, canonical edge tracking,
//! and process metadata. All resource modules register into this registry;
//! no private registries.
//!
//! When `diagnostics` is disabled, all operations compile away to no-ops
//! and `emit_graph()` returns an empty snapshot.

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(feature = "diagnostics")]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub(crate) use disabled::*;
#[cfg(feature = "diagnostics")]
pub(crate) use enabled::*;
