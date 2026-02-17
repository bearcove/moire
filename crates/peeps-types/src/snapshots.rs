use facet::Facet;

use crate::{Edge, Entity, Event, Scope};

/// A snapshot is a point-in-time process envelope of graph state.
#[derive(Facet)]
pub struct Snapshot {
    /// Runtime entities present in this snapshot.
    pub entities: Vec<Entity>,
    /// Execution scopes present in this snapshot.
    pub scopes: Vec<Scope>,
    /// Entity-to-entity edges present in this snapshot.
    pub edges: Vec<Edge>,
    /// Point-in-time events captured for this snapshot.
    pub events: Vec<Event>,
}
