use facet::Facet;

use crate::{Edge, Entity, Event, Scope, SourceId};

/// A resolved source mapping entry for interned source IDs referenced in a snapshot.
#[derive(Facet)]
pub struct SnapshotSource {
    /// Interned source identifier referenced by entities/scopes/edges/events.
    pub id: SourceId,
    /// Absolute UTF-8 path to the source file.
    pub path: String,
    /// 1-based source line.
    pub line: u32,
    /// Crate name owning this source location.
    pub krate: String,
}

/// A snapshot is a point-in-time process envelope of graph state.
#[derive(Facet)]
pub struct Snapshot {
    /// Resolved source table for every `SourceId` referenced in this snapshot.
    pub sources: Vec<SnapshotSource>,
    /// Runtime entities present in this snapshot.
    pub entities: Vec<Entity>,
    /// Execution scopes present in this snapshot.
    pub scopes: Vec<Scope>,
    /// Entity-to-entity edges present in this snapshot.
    pub edges: Vec<Edge>,
    /// Point-in-time events captured for this snapshot.
    pub events: Vec<Event>,
}
