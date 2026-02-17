//! Core graph nomenclature used across Peep's runtime model.
//!
//! - `Event`: a point-in-time occurrence with a timestamp.
//! - `Entity`: a runtime thing that exists over time (for example a lock,
//!   future, channel, request, or connection).
//! - `Edge`: a causal dependency relationship between entities.
//! - `Scope`: an execution container that groups entities (for example a
//!   process, thread, or task).
//!
//! In short: events happen to entities, entities are connected by edges,
//! and entities live inside scopes.

use facet::Facet;
use std::sync::OnceLock;
use std::time::Instant;

////////////////////////////////////////////////////////////////////////////////////
// Timestamps
////////////////////////////////////////////////////////////////////////////////////

/// First-use monotonic anchor for process-relative timestamps.
/// "Process birth" is defined as the first call to `PTime::now()`.
fn ptime_anchor() -> &'static Instant {
    static PTIME_ANCHOR: OnceLock<Instant> = OnceLock::new();
    PTIME_ANCHOR.get_or_init(Instant::now)
}

/// process start time + N milliseconds
#[derive(Facet)]
pub struct PTime(u64);

impl PTime {
    pub fn now() -> Self {
        let elapsed_ms = ptime_anchor().elapsed().as_millis().min(u64::MAX as u128) as u64;
        Self(elapsed_ms)
    }
}

////////////////////////////////////////////////////////////////////////////////////
// Scopes
////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////
// Entities
////////////////////////////////////////////////////////////////////////////////////

/// A: future, a lock, a channel end (tx, rx), a connection leg, a socket, etc.
#[derive(Facet)]
pub struct Entity {
    /// Usually `{kind}:{ulid}`
    pub id: String,

    /// When we first started tracking this entity
    pub birth: PTime,

    /// More specific info about the entity (depending on its kind)
    pub body: EntityBody,
}

////////////////////////////////////////////////////////////////////////////////////
// Edges
////////////////////////////////////////////////////////////////////////////////////

////////////////////////////////////////////////////////////////////////////////////
// Events
////////////////////////////////////////////////////////////////////////////////////

#[derive(Facet)]
pub struct Event {}
