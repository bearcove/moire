use super::*;

use peeps_types::{
    BufferState, ChannelCloseCause, ChannelClosedEvent, ChannelDetails, ChannelEndpointEntity,
    ChannelEndpointLifecycle, ChannelReceiveEvent, ChannelReceiveOutcome, ChannelSendEvent,
    ChannelSendOutcome, ChannelWaitKind, EdgeKind, EntityBody, EntityId, Event, EventTarget,
    MpscChannelDetails, OperationKind,
};
use std::future::Future;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;
use tokio::sync::mpsc;

pub(super) struct ChannelRuntimeState {
    pub(super) tx_id: EntityId,
    pub(super) rx_id: EntityId,
    pub(super) tx_ref_count: u32,
    pub(super) rx_state: ReceiverState,
    pub(super) queue_len: u32,
    pub(super) capacity: Option<u32>,
    pub(super) tx_close_cause: Option<ChannelCloseCause>,
    pub(super) rx_close_cause: Option<ChannelCloseCause>,
}

pub(super) enum ReceiverState {
    Alive,
    Dropped,
}

impl ChannelRuntimeState {
    pub(super) fn tx_lifecycle(&self) -> ChannelEndpointLifecycle {
        match self.tx_close_cause {
            Some(cause) => ChannelEndpointLifecycle::Closed(cause),
            None => ChannelEndpointLifecycle::Open,
        }
    }

    pub(super) fn rx_lifecycle(&self) -> ChannelEndpointLifecycle {
        match self.rx_close_cause {
            Some(cause) => ChannelEndpointLifecycle::Closed(cause),
            None => ChannelEndpointLifecycle::Open,
        }
    }

    pub(super) fn is_send_full(&self) -> bool {
        self.capacity
            .map(|capacity| self.queue_len >= capacity)
            .unwrap_or(false)
    }

    pub(super) fn is_receive_empty(&self) -> bool {
        self.queue_len == 0
    }
}

pub struct Sender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
    handle: EntityHandle,
    channel: Arc<StdMutex<ChannelRuntimeState>>,
    name: String,
}

pub struct Receiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
    handle: EntityHandle,
    channel: Arc<StdMutex<ChannelRuntimeState>>,
    name: String,
}

pub struct UnboundedSender<T> {
    inner: tokio::sync::mpsc::UnboundedSender<T>,
    handle: EntityHandle,
    channel: Arc<StdMutex<ChannelRuntimeState>>,
    name: String,
}

pub struct UnboundedReceiver<T> {
    inner: tokio::sync::mpsc::UnboundedReceiver<T>,
    handle: EntityHandle,
    channel: Arc<StdMutex<ChannelRuntimeState>>,
    name: String,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        if let Ok(mut state) = self.channel.lock() {
            state.tx_ref_count = state.tx_ref_count.saturating_add(1);
        }
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
            channel: self.channel.clone(),
            name: self.name.clone(),
        }
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        if let Ok(mut state) = self.channel.lock() {
            state.tx_ref_count = state.tx_ref_count.saturating_add(1);
        }
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
            channel: self.channel.clone(),
            name: self.name.clone(),
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut emit_for_rx = None;
        if let Ok(mut state) = self.channel.lock() {
            state.tx_ref_count = state.tx_ref_count.saturating_sub(1);
            if state.tx_ref_count == 0 {
                if state.tx_close_cause.is_none() {
                    state.tx_close_cause = Some(ChannelCloseCause::SenderDropped);
                }
                if state.rx_close_cause.is_none() {
                    state.rx_close_cause = Some(ChannelCloseCause::SenderDropped);
                    emit_for_rx = Some(EntityId::new(state.rx_id.as_str()));
                }
            }
        }
        apply_channel_state(&self.channel);
        if let Some(rx_id) = emit_for_rx {
            emit_channel_closed(&rx_id, ChannelCloseCause::SenderDropped);
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut emit_for_tx = None;
        if let Ok(mut state) = self.channel.lock() {
            if matches!(state.rx_state, ReceiverState::Alive) {
                state.rx_state = ReceiverState::Dropped;
                if state.tx_close_cause.is_none() {
                    state.tx_close_cause = Some(ChannelCloseCause::ReceiverDropped);
                    emit_for_tx = Some(EntityId::new(state.tx_id.as_str()));
                }
                if state.rx_close_cause.is_none() {
                    state.rx_close_cause = Some(ChannelCloseCause::ReceiverDropped);
                }
            }
        }
        apply_channel_state(&self.channel);
        if let Some(tx_id) = emit_for_tx {
            emit_channel_closed(&tx_id, ChannelCloseCause::ReceiverDropped);
        }
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        let mut emit_for_rx = None;
        if let Ok(mut state) = self.channel.lock() {
            state.tx_ref_count = state.tx_ref_count.saturating_sub(1);
            if state.tx_ref_count == 0 {
                if state.tx_close_cause.is_none() {
                    state.tx_close_cause = Some(ChannelCloseCause::SenderDropped);
                }
                if state.rx_close_cause.is_none() {
                    state.rx_close_cause = Some(ChannelCloseCause::SenderDropped);
                    emit_for_rx = Some(EntityId::new(state.rx_id.as_str()));
                }
            }
        }
        apply_channel_state(&self.channel);
        if let Some(rx_id) = emit_for_rx {
            emit_channel_closed(&rx_id, ChannelCloseCause::SenderDropped);
        }
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        let mut emit_for_tx = None;
        if let Ok(mut state) = self.channel.lock() {
            if matches!(state.rx_state, ReceiverState::Alive) {
                state.rx_state = ReceiverState::Dropped;
                if state.tx_close_cause.is_none() {
                    state.tx_close_cause = Some(ChannelCloseCause::ReceiverDropped);
                    emit_for_tx = Some(EntityId::new(state.tx_id.as_str()));
                }
                if state.rx_close_cause.is_none() {
                    state.rx_close_cause = Some(ChannelCloseCause::ReceiverDropped);
                }
            }
        }
        apply_channel_state(&self.channel);
        if let Some(tx_id) = emit_for_tx {
            emit_channel_closed(&tx_id, ChannelCloseCause::ReceiverDropped);
        }
    }
}

impl<T> Sender<T> {
    #[doc(hidden)]
    #[track_caller]
    pub fn handle(&self) -> &EntityHandle {
        &self.handle
    }

    #[track_caller]
    pub fn try_send(&self, value: T) -> Result<(), mpsc::error::TrySendError<T>> {
        self.inner.try_send(value)
    }

    #[track_caller]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    #[track_caller]
    #[allow(clippy::manual_async_fn)]
    pub fn send_with_cx(
        &self,
        value: T,
        cx: SourceLeft,
    ) -> impl Future<Output = Result<(), mpsc::error::SendError<T>>> + '_ {
        self.send_with_source(value, cx.join(SourceRight::caller()))
    }

    #[allow(clippy::manual_async_fn)]
    pub fn send_with_source(
        &self,
        value: T,
        source: Source,
    ) -> impl Future<Output = Result<(), mpsc::error::SendError<T>>> + '_ {
        async move {
            let wait_kind = self.channel.lock().ok().and_then(|state| {
                if state.is_send_full() {
                    if let Ok(event) = Event::channel_sent(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelSendEvent {
                            outcome: ChannelSendOutcome::Full,
                            queue_len: Some(state.queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Some(ChannelWaitKind::SendFull)
                } else {
                    None
                }
            });
            let wait_started = wait_kind.map(|kind| {
                emit_channel_wait_started(self.handle.id(), kind, &source);
                Instant::now()
            });

            let result = instrument_operation_on_with_source(
                &self.handle,
                OperationKind::Send,
                self.inner.send(value),
                &source,
            )
            .await;

            if let (Some(kind), Some(started)) = (wait_kind, wait_started) {
                emit_channel_wait_ended(self.handle.id(), kind, started, &source);
            }

            match result {
                Ok(()) => {
                    let queue_len = if let Ok(mut state) = self.channel.lock() {
                        state.queue_len = state.queue_len.saturating_add(1);
                        state.queue_len
                    } else {
                        0
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_sent(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelSendEvent {
                            outcome: ChannelSendOutcome::Ok,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Ok(())
                }
                Err(err) => {
                    let (queue_len, close_cause) = if let Ok(mut state) = self.channel.lock() {
                        if state.tx_close_cause.is_none() {
                            state.tx_close_cause = Some(ChannelCloseCause::ReceiverClosed);
                        }
                        if state.rx_close_cause.is_none() {
                            state.rx_close_cause = Some(ChannelCloseCause::ReceiverClosed);
                        }
                        (
                            state.queue_len,
                            state
                                .tx_close_cause
                                .unwrap_or(ChannelCloseCause::ReceiverClosed),
                        )
                    } else {
                        (0, ChannelCloseCause::ReceiverClosed)
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_sent(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelSendEvent {
                            outcome: ChannelSendOutcome::Closed,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    if let Ok(event) = Event::channel_closed(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelClosedEvent { cause: close_cause },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Err(err)
                }
            }
        }
    }
}

impl<T> Receiver<T> {
    #[doc(hidden)]
    #[track_caller]
    pub fn handle(&self) -> &EntityHandle {
        &self.handle
    }

    #[track_caller]
    #[allow(clippy::manual_async_fn)]
    pub fn recv_with_cx(&mut self, cx: SourceLeft) -> impl Future<Output = Option<T>> + '_ {
        self.recv_with_source(cx.join(SourceRight::caller()))
    }

    #[allow(clippy::manual_async_fn)]
    pub fn recv_with_source(&mut self, source: Source) -> impl Future<Output = Option<T>> + '_ {
        async move {
            let wait_kind = self.channel.lock().ok().and_then(|state| {
                if state.is_receive_empty() {
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Empty,
                            queue_len: Some(state.queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Some(ChannelWaitKind::ReceiveEmpty)
                } else {
                    None
                }
            });
            let wait_started = wait_kind.map(|kind| {
                emit_channel_wait_started(self.handle.id(), kind, &source);
                Instant::now()
            });

            let result = instrument_operation_on_with_source(
                &self.handle,
                OperationKind::Recv,
                self.inner.recv(),
                &source,
            )
            .await;

            if let (Some(kind), Some(started)) = (wait_kind, wait_started) {
                emit_channel_wait_ended(self.handle.id(), kind, started, &source);
            }

            match result {
                Some(value) => {
                    let queue_len = if let Ok(mut state) = self.channel.lock() {
                        state.queue_len = state.queue_len.saturating_sub(1);
                        state.queue_len
                    } else {
                        0
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Ok,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Some(value)
                }
                None => {
                    let (queue_len, close_cause) = if let Ok(mut state) = self.channel.lock() {
                        if state.tx_close_cause.is_none() {
                            state.tx_close_cause = Some(ChannelCloseCause::SenderDropped);
                        }
                        if state.rx_close_cause.is_none() {
                            state.rx_close_cause = Some(ChannelCloseCause::SenderDropped);
                        }
                        (
                            state.queue_len,
                            state
                                .rx_close_cause
                                .unwrap_or(ChannelCloseCause::SenderDropped),
                        )
                    } else {
                        (0, ChannelCloseCause::SenderDropped)
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Closed,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    if let Ok(event) = Event::channel_closed(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelClosedEvent { cause: close_cause },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    None
                }
            }
        }
    }
}

impl<T> UnboundedSender<T> {
    #[doc(hidden)]
    #[track_caller]
    pub fn handle(&self) -> &EntityHandle {
        &self.handle
    }

    #[track_caller]
    pub fn send_with_cx(&self, value: T, cx: SourceLeft) -> Result<(), mpsc::error::SendError<T>> {
        self.send_with_source(value, cx.join(SourceRight::caller()))
    }

    pub fn send_with_source(
        &self,
        value: T,
        source: Source,
    ) -> Result<(), mpsc::error::SendError<T>> {
        match self.inner.send(value) {
            Ok(()) => {
                let queue_len = if let Ok(mut state) = self.channel.lock() {
                    state.queue_len = state.queue_len.saturating_add(1);
                    state.queue_len
                } else {
                    0
                };
                apply_channel_state(&self.channel);
                if let Ok(event) = Event::channel_sent(
                    EventTarget::Entity(self.handle.id().clone()),
                    &ChannelSendEvent {
                        outcome: ChannelSendOutcome::Ok,
                        queue_len: Some(queue_len),
                    },
                ) {
                    record_event_with_source(event, &source);
                }
                Ok(())
            }
            Err(err) => {
                let (queue_len, close_cause) = if let Ok(mut state) = self.channel.lock() {
                    if state.tx_close_cause.is_none() {
                        state.tx_close_cause = Some(ChannelCloseCause::ReceiverClosed);
                    }
                    if state.rx_close_cause.is_none() {
                        state.rx_close_cause = Some(ChannelCloseCause::ReceiverClosed);
                    }
                    (
                        state.queue_len,
                        state
                            .tx_close_cause
                            .unwrap_or(ChannelCloseCause::ReceiverClosed),
                    )
                } else {
                    (0, ChannelCloseCause::ReceiverClosed)
                };
                apply_channel_state(&self.channel);
                if let Ok(event) = Event::channel_sent(
                    EventTarget::Entity(self.handle.id().clone()),
                    &ChannelSendEvent {
                        outcome: ChannelSendOutcome::Closed,
                        queue_len: Some(queue_len),
                    },
                ) {
                    record_event_with_source(event, &source);
                }
                if let Ok(event) = Event::channel_closed(
                    EventTarget::Entity(self.handle.id().clone()),
                    &ChannelClosedEvent { cause: close_cause },
                ) {
                    record_event_with_source(event, &source);
                }
                Err(err)
            }
        }
    }

    #[track_caller]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

impl<T> UnboundedReceiver<T> {
    #[doc(hidden)]
    #[track_caller]
    pub fn handle(&self) -> &EntityHandle {
        &self.handle
    }

    #[track_caller]
    #[allow(clippy::manual_async_fn)]
    pub fn recv_with_cx(&mut self, cx: SourceLeft) -> impl Future<Output = Option<T>> + '_ {
        self.recv_with_source(cx.join(SourceRight::caller()))
    }

    #[allow(clippy::manual_async_fn)]
    pub fn recv_with_source(&mut self, source: Source) -> impl Future<Output = Option<T>> + '_ {
        async move {
            let wait_kind = self.channel.lock().ok().and_then(|state| {
                if state.is_receive_empty() {
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Empty,
                            queue_len: Some(state.queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Some(ChannelWaitKind::ReceiveEmpty)
                } else {
                    None
                }
            });
            let wait_started = wait_kind.map(|kind| {
                emit_channel_wait_started(self.handle.id(), kind, &source);
                Instant::now()
            });

            let result = instrument_operation_on_with_source(
                &self.handle,
                OperationKind::Recv,
                self.inner.recv(),
                &source,
            )
            .await;

            if let (Some(kind), Some(started)) = (wait_kind, wait_started) {
                emit_channel_wait_ended(self.handle.id(), kind, started, &source);
            }

            match result {
                Some(value) => {
                    let queue_len = if let Ok(mut state) = self.channel.lock() {
                        state.queue_len = state.queue_len.saturating_sub(1);
                        state.queue_len
                    } else {
                        0
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Ok,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    Some(value)
                }
                None => {
                    let (queue_len, close_cause) = if let Ok(mut state) = self.channel.lock() {
                        if state.tx_close_cause.is_none() {
                            state.tx_close_cause = Some(ChannelCloseCause::SenderDropped);
                        }
                        if state.rx_close_cause.is_none() {
                            state.rx_close_cause = Some(ChannelCloseCause::SenderDropped);
                        }
                        (
                            state.queue_len,
                            state
                                .rx_close_cause
                                .unwrap_or(ChannelCloseCause::SenderDropped),
                        )
                    } else {
                        (0, ChannelCloseCause::SenderDropped)
                    };
                    apply_channel_state(&self.channel);
                    if let Ok(event) = Event::channel_received(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelReceiveEvent {
                            outcome: ChannelReceiveOutcome::Closed,
                            queue_len: Some(queue_len),
                        },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    if let Ok(event) = Event::channel_closed(
                        EventTarget::Entity(self.handle.id().clone()),
                        &ChannelClosedEvent { cause: close_cause },
                    ) {
                        record_event_with_source(event, &source);
                    }
                    None
                }
            }
        }
    }
}

pub fn channel<T>(
    name: impl Into<String>,
    capacity: usize,
    source: SourceRight,
) -> (Sender<T>, Receiver<T>) {
    let name: String = name.into().into();
    let (tx, rx) = mpsc::channel(capacity);
    let capacity_u32 = capacity.min(u32::MAX as usize) as u32;

    let details = ChannelDetails::Mpsc(MpscChannelDetails {
        buffer: Some(BufferState {
            occupancy: 0,
            capacity: Some(capacity_u32),
        }),
    });
    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::ChannelTx(ChannelEndpointEntity {
            lifecycle: ChannelEndpointLifecycle::Open,
            details,
        }),
        source,
    );
    let details = ChannelDetails::Mpsc(MpscChannelDetails {
        buffer: Some(BufferState {
            occupancy: 0,
            capacity: Some(capacity_u32),
        }),
    });
    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::ChannelRx(ChannelEndpointEntity {
            lifecycle: ChannelEndpointLifecycle::Open,
            details,
        }),
        source,
    );
    tx_handle.link_to_handle(&rx_handle, EdgeKind::ChannelLink);
    let channel = Arc::new(StdMutex::new(ChannelRuntimeState {
        tx_id: tx_handle.id().clone(),
        rx_id: rx_handle.id().clone(),
        tx_ref_count: 1,
        rx_state: ReceiverState::Alive,
        queue_len: 0,
        capacity: Some(capacity_u32),
        tx_close_cause: None,
        rx_close_cause: None,
    }));

    (
        Sender {
            inner: tx,
            handle: tx_handle,
            channel: channel.clone(),
            name: name.clone(),
        },
        Receiver {
            inner: rx,
            handle: rx_handle,
            channel,
            name,
        },
    )
}

pub fn unbounded_channel<T>(
    name: impl Into<String>,
    source: SourceRight,
) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let name: String = name.into().into();
    let (tx, rx) = mpsc::unbounded_channel();
    let details = ChannelDetails::Mpsc(MpscChannelDetails {
        buffer: Some(BufferState {
            occupancy: 0,
            capacity: None,
        }),
    });
    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::ChannelTx(ChannelEndpointEntity {
            lifecycle: ChannelEndpointLifecycle::Open,
            details,
        }),
        source,
    );
    let details = ChannelDetails::Mpsc(MpscChannelDetails {
        buffer: Some(BufferState {
            occupancy: 0,
            capacity: None,
        }),
    });
    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::ChannelRx(ChannelEndpointEntity {
            lifecycle: ChannelEndpointLifecycle::Open,
            details,
        }),
        source,
    );
    tx_handle.link_to_handle(&rx_handle, EdgeKind::ChannelLink);
    let channel = Arc::new(StdMutex::new(ChannelRuntimeState {
        tx_id: tx_handle.id().clone(),
        rx_id: rx_handle.id().clone(),
        tx_ref_count: 1,
        rx_state: ReceiverState::Alive,
        queue_len: 0,
        capacity: None,
        tx_close_cause: None,
        rx_close_cause: None,
    }));
    (
        UnboundedSender {
            inner: tx,
            handle: tx_handle,
            channel: channel.clone(),
            name: name.clone(),
        },
        UnboundedReceiver {
            inner: rx,
            handle: rx_handle,
            channel,
            name,
        },
    )
}

impl<T> AsEntityRef for Sender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}

impl<T> AsEntityRef for Receiver<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}

impl<T> AsEntityRef for UnboundedSender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}

impl<T> AsEntityRef for UnboundedReceiver<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}
