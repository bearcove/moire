// r[impl process.dependency]
//! Shim crate that re-exports the target backend.
//!
//! - Native targets: `moire-tokio`
//! - wasm32 targets: `moire-wasm`

// r[impl api.backend.native]
#[cfg(not(target_arch = "wasm32"))]
pub use moire_tokio::*;
// r[impl api.backend.wasm]
#[cfg(target_arch = "wasm32")]
pub use moire_wasm::*;
