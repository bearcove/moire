// r[impl api.mpsc]
use super::capture_backtrace_id;

use moire_runtime::{
    instrument_operation_on_with_source, record_event_with_source, AsEntityRef, EntityHandle,
    EntityRef, WeakEntityHandle,
};
use moire_types::{
    EdgeKind, EntityBody, Event, EventKind, EventTarget, MpscRxEntity, MpscTxEntity,
};
use tokio::sync::mpsc;

pub struct Sender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
    handle: EntityHandle<moire_types::MpscTx>,
}

pub struct Receiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
    handle: EntityHandle<moire_types::MpscRx>,
    tx_handle: WeakEntityHandle<moire_types::MpscTx>,
}

pub struct UnboundedSender<T> {
    inner: tokio::sync::mpsc::UnboundedSender<T>,
    handle: EntityHandle<moire_types::MpscTx>,
}

pub struct UnboundedReceiver<T> {
    inner: tokio::sync::mpsc::UnboundedReceiver<T>,
    handle: EntityHandle<moire_types::MpscRx>,
    tx_handle: WeakEntityHandle<moire_types::MpscTx>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
        }
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
        }
    }
}

impl<T> Sender<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::MpscTx> {
        &self.handle
    }

    pub fn try_send(&self, value: T) -> Result<(), mpsc::error::TrySendError<T>> {
        match self.inner.try_send(value) {
            Ok(()) => {
                let _ = self
                    .handle
                    .mutate(|body| body.queue_len = body.queue_len.saturating_add(1));
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }    pub async fn send(&self, value: T) -> Result<(), mpsc::error::SendError<T>> {
        let source = capture_backtrace_id();
        let result =
            instrument_operation_on_with_source(&self.handle, self.inner.send(value), source).await;
        if result.is_ok() {
            let _ = self
                .handle
                .mutate(|body| body.queue_len = body.queue_len.saturating_add(1));
        }
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelSent,
            source,
        );
        record_event_with_source(event, source);
        result
    }
}

impl<T> Receiver<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::MpscRx> {
        &self.handle
    }    pub async fn recv(&mut self) -> Option<T> {
        let source = capture_backtrace_id();
        let result =
            instrument_operation_on_with_source(&self.handle, self.inner.recv(), source).await;
        if result.is_some() {
            let _ = self
                .tx_handle
                .mutate(|body| body.queue_len = body.queue_len.saturating_sub(1));
        }
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelReceived,
            source,
        );
        record_event_with_source(event, source);
        result
    }

    pub fn close(&mut self) {
        self.inner.close();
    }
}

impl<T> UnboundedSender<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::MpscTx> {
        &self.handle
    }    pub fn send(&self, value: T) -> Result<(), mpsc::error::SendError<T>> {
        let source = capture_backtrace_id();
        match self.inner.send(value) {
            Ok(()) => {
                let _ = self
                    .handle
                    .mutate(|body| body.queue_len = body.queue_len.saturating_add(1));
                let event = Event::new_with_source(
                    EventTarget::Entity(self.handle.id().clone()),
                    EventKind::ChannelSent,
                    source,
                );
                record_event_with_source(event, source);
                Ok(())
            }
            Err(err) => {
                let event = Event::new_with_source(
                    EventTarget::Entity(self.handle.id().clone()),
                    EventKind::ChannelSent,
                    source,
                );
                record_event_with_source(event, source);
                Err(err)
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

impl<T> UnboundedReceiver<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::MpscRx> {
        &self.handle
    }    pub async fn recv(&mut self) -> Option<T> {
        let source = capture_backtrace_id();
        let result =
            instrument_operation_on_with_source(&self.handle, self.inner.recv(), source).await;
        if result.is_some() {
            let _ = self
                .tx_handle
                .mutate(|body| body.queue_len = body.queue_len.saturating_sub(1));
        }
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelReceived,
            source,
        );
        record_event_with_source(event, source);
        result
    }

    pub fn close(&mut self) {
        self.inner.close();
    }
}

pub fn channel<T>(name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    let source = capture_backtrace_id();
    let name = name.into();
    let (tx, rx) = mpsc::channel(capacity);
    let capacity_u32 = capacity.min(u32::MAX as usize) as u32;

    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::MpscTx(MpscTxEntity {
            queue_len: 0,
            capacity: Some(capacity_u32),
        }),
        source,
    )
    .into_typed::<moire_types::MpscTx>();

    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::MpscRx(MpscRxEntity {}),
        source,
    )
    .into_typed::<moire_types::MpscRx>();

    tx_handle.link_to_handle_with_source(&rx_handle, EdgeKind::PairedWith, source);

    (
        Sender {
            inner: tx,
            handle: tx_handle.clone(),
        },
        Receiver {
            inner: rx,
            handle: rx_handle,
            tx_handle: tx_handle.downgrade(),
        },
    )
}

pub fn mpsc_channel<T>(name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    channel(name, capacity)
}

pub fn unbounded_channel<T>(name: impl Into<String>) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let source = capture_backtrace_id();
    let name = name.into();
    let (tx, rx) = mpsc::unbounded_channel();

    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::MpscTx(MpscTxEntity {
            queue_len: 0,
            capacity: None,
        }),
        source,
    )
    .into_typed::<moire_types::MpscTx>();

    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::MpscRx(MpscRxEntity {}),
        source,
    )
    .into_typed::<moire_types::MpscRx>();

    tx_handle.link_to_handle_with_source(&rx_handle, EdgeKind::PairedWith, source);

    (
        UnboundedSender {
            inner: tx,
            handle: tx_handle.clone(),
        },
        UnboundedReceiver {
            inner: rx,
            handle: rx_handle,
            tx_handle: tx_handle.downgrade(),
        },
    )
}

pub fn mpsc_unbounded_channel<T>(name: impl Into<String>) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    unbounded_channel(name)
}

impl<T> AsEntityRef for Sender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}

impl<T> AsEntityRef for UnboundedSender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}
