use facet::Facet;
use moire_source::SourceId;

use crate::{next_event_id, EntityId, EventId, PTime, ScopeId};

// r[impl model.event.fields]
#[derive(Facet)]
pub struct Event {
    /// Opaque event identifier.
    pub id: EventId,

    /// Event timestamp.
    pub at: PTime,

    /// Event source.
    pub source: SourceId,

    /// Event target (entity or scope).
    pub target: EventTarget,

    /// Event kind.
    pub kind: EventKind,
}

impl Event {
    /// Builds an event with explicit source context.
    pub fn new_with_source(target: EventTarget, kind: EventKind, source: SourceId) -> Self {
        Self {
            id: next_event_id(),
            at: PTime::now(),
            source,
            target,
            kind,
        }
    }
}

#[derive(Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum EventTarget {
    Entity(EntityId),
    Scope(ScopeId),
}

// r[impl model.event.kinds]
#[derive(Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum EventKind {
    StateChanged,
    ChannelSent,
    ChannelReceived,
}

crate::impl_sqlite_json!(EventTarget);
crate::impl_sqlite_json!(EventKind);

crate::declare_event_target_slots!(
    EntityTargetSlot::Entity(EntityId),
    ScopeTargetSlot::Scope(ScopeId),
);

crate::declare_event_kind_slots!(
    StateChangedKindSlot::StateChanged,
    ChannelSentKindSlot::ChannelSent,
    ChannelReceivedKindSlot::ChannelReceived,
);

#[derive(Facet)]
pub struct ChannelSentEvent {
    /// Observed wait duration in nanoseconds, if this operation suspended.
    pub wait_ns: Option<u64>,
    /// True if the send failed because the other side was gone.
    pub closed: bool,
}

#[derive(Facet)]
pub struct ChannelReceivedEvent {
    /// Observed wait duration in nanoseconds, if this operation suspended.
    pub wait_ns: Option<u64>,
    /// True if the recv failed because the other side was gone.
    pub closed: bool,
}
