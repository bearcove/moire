//! Shim crate that re-exports the target backend.
//!
//! - Native targets: `moire-tokio`
//! - wasm32 targets: `moire-wasm`

#[cfg(not(target_arch = "wasm32"))]
pub use moire_tokio::*;
#[cfg(target_arch = "wasm32")]
pub use moire_wasm::*;

#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub use moire_tokio as __backend;
#[doc(hidden)]
#[cfg(target_arch = "wasm32")]
pub use moire_wasm as __backend;

#[macro_export]
macro_rules! facade {
    () => {
        $crate::__backend::facade!();
    };
}
