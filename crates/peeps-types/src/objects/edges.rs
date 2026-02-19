use facet::Facet;
use peeps_source::SourceId;

use crate::EntityId;

/// Relationship between two entities.
#[derive(Facet)]
pub struct Edge {
    /// Source entity in the causal relationship.
    pub src: EntityId,

    /// Destination entity in the causal relationship.
    pub dst: EntityId,

    /// Location in source code and crate information.
    pub source: SourceId,

    /// Causal edge kind.
    pub kind: EdgeKind,
}

impl Edge {
    /// Builds a causal edge.
    pub fn new(src: EntityId, dst: EntityId, kind: EdgeKind, source: impl Into<SourceId>) -> Self {
        Self {
            src,
            dst,
            source: source.into(),
            kind,
        }
    }
}

#[derive(Facet, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Contextual resource-touch relationship (actor has interacted with resource).
    Touches,

    /// Polled relationship (non-blocking observation of dependency).
    Polls,

    /// Waiting/blocked-on relationship.
    Needs,

    /// Resource ownership relationship (resource -> current holder).
    Holds,

    /// Closure/cancellation cause relationship.
    ClosedBy,

    /// Structural channel endpoint pairing (`tx -> rx`).
    ChannelLink,

    /// Structural request/response pairing.
    RpcLink,
}

/// Primitive operation represented on an operation edge.
#[derive(Facet, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum OperationKind {
    Send,
    Recv,
    Acquire,
    Lock,
    NotifyWait,
    OncecellWait,
}

/// Runtime state for a primitive operation edge.
#[derive(Facet, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum OperationState {
    Active,
    Pending,
    Done,
    Failed,
    Cancelled,
}

/// Metadata payload for operation edges (`EdgeKind::Needs` + `op_kind`).
#[derive(Facet, Clone, Debug, PartialEq)]
pub struct OperationEdgeMeta {
    pub op_kind: OperationKind,
    pub state: OperationState,
    pub pending_since_ptime_ms: Option<u64>,
    pub last_change_ptime_ms: u64,
    pub source: String,
    pub krate: Option<String>,
    pub poll_count: Option<u64>,
    pub details: Option<facet_value::Value>,
}
