//! Instrumented task spawning and management, mirroring [`tokio::task`].
//!
//! This module mirrors the structure of `tokio::task` and can be used as a
//! drop-in replacement. Spawned tasks and join sets are registered as named
//! entities in the Moiré runtime graph, so the dashboard can show the full
//! dependency graph between tasks and what each one is currently waiting on.
//!
//! # Available items
//!
//! | Item | Tokio equivalent |
//! |---|---|
//! | [`JoinSet`] | `tokio::task::JoinSet` |

pub mod joinset;

pub use self::joinset::*;
