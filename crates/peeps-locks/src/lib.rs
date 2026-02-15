//! Diagnostic wrappers for locks (both sync and async).
//!
//! When the `diagnostics` feature is **disabled**, these are zero-cost wrappers
//! that compile down to plain locks. When **enabled**, every lock registers
//! itself in a global registry and tracks:
//!
//! - Who is waiting to acquire the lock (with backtrace + duration)
//! - Who currently holds the lock (with backtrace + duration)
//!
//! ## Sync locks (`DiagnosticRwLock`, `DiagnosticMutex`)
//! Wrappers around `parking_lot::RwLock` and `parking_lot::Mutex`.
//!
//! ## Async locks (`DiagnosticAsyncRwLock`, `DiagnosticAsyncMutex`)
//! Wrappers around `tokio::sync::RwLock` and `tokio::sync::Mutex`.
//! Uses a `WaiterToken` to handle cancellation — if a `.await` is dropped,
//! the waiter entry is automatically cleaned up.

#[cfg(feature = "diagnostics")]
mod enabled;
#[cfg(not(feature = "diagnostics"))]
mod disabled;

#[cfg(feature = "diagnostics")]
pub use enabled::*;
#[cfg(not(feature = "diagnostics"))]
pub use disabled::*;

pub use peeps_types::{
    LockAcquireKind, LockHolderSnapshot, LockInfoSnapshot, LockSnapshot, LockWaiterSnapshot,
};
