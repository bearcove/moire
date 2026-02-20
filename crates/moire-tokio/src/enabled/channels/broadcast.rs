// r[impl api.broadcast]
use super::capture_backtrace_id;

use moire_runtime::{
    record_event_with_source, AsEntityRef, EntityHandle, EntityRef, WeakEntityHandle,
};
use moire_types::{
    BroadcastRxEntity, BroadcastTxEntity, EdgeKind, EntityBody, Event, EventKind, EventTarget,
};
use tokio::sync::broadcast;

pub struct BroadcastSender<T> {
    inner: tokio::sync::broadcast::Sender<T>,
    handle: EntityHandle<moire_types::BroadcastTx>,
}

pub struct BroadcastReceiver<T> {
    inner: tokio::sync::broadcast::Receiver<T>,
    handle: EntityHandle<moire_types::BroadcastRx>,
    tx_handle: WeakEntityHandle<moire_types::BroadcastTx>,
}

impl<T> Clone for BroadcastSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
        }
    }
}

impl<T: Clone> Clone for BroadcastReceiver<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.resubscribe(),
            handle: self.handle.clone(),
            tx_handle: self.tx_handle.clone(),
        }
    }
}

impl<T: Clone> BroadcastSender<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::BroadcastTx> {
        &self.handle
    }
    pub fn subscribe(&self) -> BroadcastReceiver<T> {
        let source = capture_backtrace_id();
        let handle = EntityHandle::new(
            "broadcast:rx.subscribe",
            EntityBody::BroadcastRx(BroadcastRxEntity { lag: 0 }),
            source,
        )
        .into_typed::<moire_types::BroadcastRx>();
        self.handle
            .link_to_handle_with_source(&handle, EdgeKind::PairedWith, source);
        BroadcastReceiver {
            inner: self.inner.subscribe(),
            handle,
            tx_handle: self.handle.downgrade(),
        }
    }
    pub fn send(&self, value: T) -> Result<usize, broadcast::error::SendError<T>> {
        let source = capture_backtrace_id();
        let result = self.inner.send(value);
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelSent,
            source,
        );
        record_event_with_source(event, source);
        result
    }
}

impl<T: Clone> BroadcastReceiver<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::BroadcastRx> {
        &self.handle
    }
    pub async fn recv(&mut self) -> Result<T, broadcast::error::RecvError> {
        let source = capture_backtrace_id();
        match self.inner.recv().await {
            Ok(value) => {
                let lag = self.inner.len().min(u32::MAX as usize) as u32;
                let _ = self.handle.mutate(|body| body.lag = lag);
                let event = Event::new_with_source(
                    EventTarget::Entity(self.handle.id().clone()),
                    EventKind::ChannelReceived,
                    source,
                );
                record_event_with_source(event, source);
                Ok(value)
            }
            Err(err) => {
                if let broadcast::error::RecvError::Lagged(n) = err {
                    let lag = n.min(u32::MAX as u64) as u32;
                    let _ = self.handle.mutate(|body| body.lag = lag);
                }
                let event = Event::new_with_source(
                    EventTarget::Entity(self.handle.id().clone()),
                    EventKind::ChannelReceived,
                    source,
                );
                record_event_with_source(event, source);
                Err(err)
            }
        }
    }
}

pub fn broadcast<T: Clone>(
    name: impl Into<String>,
    capacity: usize,
) -> (BroadcastSender<T>, BroadcastReceiver<T>) {
    let source = capture_backtrace_id();
    let name = name.into();
    let (tx, rx) = broadcast::channel(capacity);
    let capacity_u32 = capacity.min(u32::MAX as usize) as u32;

    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::BroadcastTx(BroadcastTxEntity {
            capacity: capacity_u32,
        }),
        source,
    )
    .into_typed::<moire_types::BroadcastTx>();

    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::BroadcastRx(BroadcastRxEntity { lag: 0 }),
        source,
    )
    .into_typed::<moire_types::BroadcastRx>();

    tx_handle.link_to_handle_with_source(&rx_handle, EdgeKind::PairedWith, source);

    (
        BroadcastSender {
            inner: tx,
            handle: tx_handle.clone(),
        },
        BroadcastReceiver {
            inner: rx,
            handle: rx_handle,
            tx_handle: tx_handle.downgrade(),
        },
    )
}

pub fn broadcast_channel<T: Clone>(
    name: impl Into<String>,
    capacity: usize,
) -> (BroadcastSender<T>, BroadcastReceiver<T>) {
    broadcast(name, capacity)
}

impl<T: Clone> AsEntityRef for BroadcastSender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}
