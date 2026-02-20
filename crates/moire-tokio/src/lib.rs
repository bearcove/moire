//! Tokio-backed moire instrumentation surface.

#[doc(hidden)]
pub use facet_value;
#[doc(hidden)]
pub use parking_lot;
#[doc(hidden)]
pub use moire_types;
#[doc(hidden)]
pub use tokio;

#[cfg(target_arch = "wasm32")]
compile_error!("`moire-tokio` is native-only; use `moire-wasm` on wasm32");

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(feature = "diagnostics")]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub use disabled::*;
#[cfg(feature = "diagnostics")]
pub use enabled::*;
