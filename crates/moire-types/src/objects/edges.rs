use facet::Facet;
use moire_trace_types::BacktraceId;

use crate::EntityId;

// r[impl model.edge.fields]
/// Relationship between two entities.
#[derive(Facet)]
pub struct Edge {
    /// Source entity in the causal relationship.
    pub src: EntityId,

    /// Destination entity in the causal relationship.
    pub dst: EntityId,

    /// Backtrace when this edge was created
    pub backtrace: BacktraceId,

    /// Causal edge kind.
    pub kind: EdgeKind,
}

impl Edge {
    /// Builds a causal edge.
    pub fn new(src: EntityId, dst: EntityId, kind: EdgeKind, backtrace: BacktraceId) -> Self {
        Self {
            src,
            dst,
            backtrace,
            kind,
        }
    }
}

// r[impl model.edge.kinds]
#[derive(Facet, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Poll relationship (task/entity is actively polling another future/resource).
    ///
    /// Example: parent future polls child future during one executor tick.
    Polls,

    /// Waiting relationship (task/entity is blocked on another resource).
    ///
    /// Example: receiver waits on channel `rx.recv().await`.
    WaitingOn,

    /// Pairing relationship between two endpoints that form one logical primitive.
    ///
    /// Example: channel sender paired with its corresponding receiver.
    PairedWith,

    /// Resource ownership/lease relationship (resource -> current holder).
    ///
    /// Example: semaphore is held_by the holder of an acquired permit.
    HeldBy,
}

crate::impl_sqlite_json!(EdgeKind);

crate::declare_edge_kind_slots!(
    PollsEdgeKindSlot::Polls,
    WaitingOnEdgeKindSlot::WaitingOn,
    PairedWithEdgeKindSlot::PairedWith,
    HeldByEdgeKindSlot::HeldBy,
);
