//! Instrumented synchronization primitives, mirroring [`tokio::sync`].
//!
//! This module mirrors the structure of `tokio::sync` and can be used as a
//! drop-in replacement. Every primitive is registered as a named entity in the
//! Moiré runtime graph so the dashboard can show which tasks are blocked on
//! which locks, semaphores, or channels — and exactly where in your code they
//! got there.
//!
//! # Available primitives
//!
//! | Submodule / type | Tokio equivalent |
//! |---|---|
//! | [`mpsc`] | [`tokio::sync::mpsc`] |
//! | [`broadcast`] | [`tokio::sync::broadcast`] |
//! | [`oneshot`] | [`tokio::sync::oneshot`] |
//! | [`watch`] | [`tokio::sync::watch`] |
//! | [`Mutex`] | [`tokio::sync::Mutex`] |
//! | [`RwLock`] | [`tokio::sync::RwLock`] |
//! | [`Semaphore`] | [`tokio::sync::Semaphore`] |
//! | [`Notify`] | [`tokio::sync::Notify`] |
//! | [`OnceCell`] | [`tokio::sync::OnceCell`] |

pub mod broadcast;
pub mod mpsc;
pub mod oneshot;
pub mod watch;

mod mutex;
pub use mutex::*;

mod notify;
pub use notify::*;

mod once_cell;
pub use once_cell::*;

mod rwlock;
pub use rwlock::*;

mod semaphore;
pub use semaphore::*;
