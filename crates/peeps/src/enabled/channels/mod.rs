use peeps_types::{
    BufferState, ChannelCloseCause, ChannelClosedEvent, ChannelEndpointLifecycle,
    ChannelWaitEndedEvent, ChannelWaitKind, ChannelWaitStartedEvent, EntityId, Event, EventTarget,
    OneshotState,
};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

pub(super) use super::{Source, SourceLeft, SourceRight};
pub(super) use peeps_runtime::runtime_db;
pub(super) use peeps_runtime::{
    instrument_operation_on_with_source, record_event_with_entity_source, record_event_with_source,
    AsEntityRef, EntityHandle, EntityRef,
};

pub(crate) mod broadcast;
use broadcast::BroadcastRuntimeState;
pub use broadcast::{BroadcastReceiver, BroadcastSender};

pub(crate) mod mpsc;
use mpsc::{ChannelRuntimeState, ReceiverState};
pub use mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};

pub(crate) mod oneshot;
use oneshot::OneshotRuntimeState;
pub use oneshot::{OneshotReceiver, OneshotSender};

pub(crate) mod watch;
use watch::WatchRuntimeState;
pub use watch::{WatchReceiver, WatchSender};

pub(super) fn sync_channel_state(
    channel: &Arc<StdMutex<ChannelRuntimeState>>,
) -> Option<(
    EntityId,
    EntityId,
    Option<BufferState>,
    ChannelEndpointLifecycle,
    ChannelEndpointLifecycle,
)> {
    let state = channel.lock().ok()?;
    Some((
        EntityId::new(state.tx_id.as_str()),
        EntityId::new(state.rx_id.as_str()),
        Some(BufferState {
            occupancy: state.queue_len,
            capacity: state.capacity,
        }),
        state.tx_lifecycle(),
        state.rx_lifecycle(),
    ))
}

pub(super) fn apply_channel_state(channel: &Arc<StdMutex<ChannelRuntimeState>>) {
    let Some((tx_id, rx_id, buffer, tx_lifecycle, rx_lifecycle)) = sync_channel_state(channel)
    else {
        return;
    };
    if let Ok(mut db) = runtime_db().lock() {
        db.update_channel_endpoint_state(&tx_id, tx_lifecycle, buffer);
        db.update_channel_endpoint_state(&rx_id, rx_lifecycle, buffer);
    }
}

pub(super) fn sync_oneshot_state(
    channel: &Arc<StdMutex<OneshotRuntimeState>>,
) -> Option<(
    EntityId,
    EntityId,
    OneshotState,
    ChannelEndpointLifecycle,
    ChannelEndpointLifecycle,
)> {
    let state = channel.lock().ok()?;
    Some((
        EntityId::new(state.tx_id.as_str()),
        EntityId::new(state.rx_id.as_str()),
        state.state,
        state.tx_lifecycle,
        state.rx_lifecycle,
    ))
}

pub(super) fn apply_oneshot_state(channel: &Arc<StdMutex<OneshotRuntimeState>>) {
    let Some((tx_id, rx_id, state, tx_lifecycle, rx_lifecycle)) = sync_oneshot_state(channel)
    else {
        return;
    };
    if let Ok(mut db) = runtime_db().lock() {
        db.update_oneshot_endpoint_state(&tx_id, tx_lifecycle, state);
        db.update_oneshot_endpoint_state(&rx_id, rx_lifecycle, state);
    }
}

pub(super) fn sync_broadcast_state(
    channel: &Arc<StdMutex<BroadcastRuntimeState>>,
) -> Option<(
    EntityId,
    EntityId,
    Option<BufferState>,
    ChannelEndpointLifecycle,
    ChannelEndpointLifecycle,
)> {
    let state = channel.lock().ok()?;
    let tx_lifecycle = match state.tx_close_cause {
        Some(cause) => ChannelEndpointLifecycle::Closed(cause),
        None => ChannelEndpointLifecycle::Open,
    };
    let rx_lifecycle = match state.rx_close_cause {
        Some(cause) => ChannelEndpointLifecycle::Closed(cause),
        None => ChannelEndpointLifecycle::Open,
    };
    Some((
        EntityId::new(state.tx_id.as_str()),
        EntityId::new(state.rx_id.as_str()),
        Some(BufferState {
            occupancy: 0,
            capacity: Some(state.capacity),
        }),
        tx_lifecycle,
        rx_lifecycle,
    ))
}

pub(super) fn apply_broadcast_state(channel: &Arc<StdMutex<BroadcastRuntimeState>>) {
    let Some((tx_id, rx_id, buffer, tx_lifecycle, rx_lifecycle)) = sync_broadcast_state(channel)
    else {
        return;
    };
    if let Ok(mut db) = runtime_db().lock() {
        db.update_channel_endpoint_state(&tx_id, tx_lifecycle, buffer);
        db.update_channel_endpoint_state(&rx_id, rx_lifecycle, buffer);
    }
}

pub(super) fn sync_watch_state(
    channel: &Arc<StdMutex<WatchRuntimeState>>,
) -> Option<(
    EntityId,
    EntityId,
    ChannelEndpointLifecycle,
    ChannelEndpointLifecycle,
    Option<peeps_types::PTime>,
)> {
    let state = channel.lock().ok()?;
    let tx_lifecycle = match state.tx_close_cause {
        Some(cause) => ChannelEndpointLifecycle::Closed(cause),
        None => ChannelEndpointLifecycle::Open,
    };
    let rx_lifecycle = match state.rx_close_cause {
        Some(cause) => ChannelEndpointLifecycle::Closed(cause),
        None => ChannelEndpointLifecycle::Open,
    };
    Some((
        EntityId::new(state.tx_id.as_str()),
        EntityId::new(state.rx_id.as_str()),
        tx_lifecycle,
        rx_lifecycle,
        state.last_update_at,
    ))
}

pub(super) fn apply_watch_state(channel: &Arc<StdMutex<WatchRuntimeState>>) {
    let Some((tx_id, rx_id, tx_lifecycle, rx_lifecycle, last_update_at)) =
        sync_watch_state(channel)
    else {
        return;
    };
    if let Ok(mut db) = runtime_db().lock() {
        db.update_channel_endpoint_state(&tx_id, tx_lifecycle, None);
        db.update_channel_endpoint_state(&rx_id, rx_lifecycle, None);
        db.update_watch_last_update(&tx_id, last_update_at);
        db.update_watch_last_update(&rx_id, last_update_at);
    }
}

pub(super) fn emit_channel_wait_started(target: &EntityId, kind: ChannelWaitKind, source: &Source) {
    if let Ok(event) = Event::channel_wait_started_with_source(
        EventTarget::Entity(target.clone()),
        &ChannelWaitStartedEvent { kind },
        source.as_str(),
        source.krate(),
    ) {
        if let Ok(mut db) = runtime_db().lock() {
            db.record_event(event);
        }
    }
}

pub(super) fn emit_channel_wait_ended(
    target: &EntityId,
    kind: ChannelWaitKind,
    started: Instant,
    source: &Source,
) {
    let wait_ns = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    if let Ok(event) = Event::channel_wait_ended_with_source(
        EventTarget::Entity(target.clone()),
        &ChannelWaitEndedEvent { kind, wait_ns },
        source.as_str(),
        source.krate(),
    ) {
        if let Ok(mut db) = runtime_db().lock() {
            db.record_event(event);
        }
    }
}

pub(super) fn emit_channel_closed(target: &EntityId, cause: ChannelCloseCause) {
    if let Ok(event) = Event::channel_closed(
        EventTarget::Entity(target.clone()),
        &ChannelClosedEvent { cause },
    ) {
        record_event_with_entity_source(event, target);
    }
}
