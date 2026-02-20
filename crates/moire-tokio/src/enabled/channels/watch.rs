use super::capture_backtrace_id;

use moire_runtime::{
    instrument_operation_on_with_source, record_event_with_source, AsEntityRef, EntityHandle,
    EntityRef, WeakEntityHandle,
};
use moire_types::{
    EdgeKind, EntityBody, Event, EventKind, EventTarget, WatchRxEntity, WatchTxEntity,
};
use tokio::sync::watch;

pub struct WatchSender<T> {
    inner: tokio::sync::watch::Sender<T>,
    handle: EntityHandle<moire_types::WatchTx>,
}

pub struct WatchReceiver<T> {
    inner: tokio::sync::watch::Receiver<T>,
    handle: EntityHandle<moire_types::WatchRx>,
    tx_handle: WeakEntityHandle<moire_types::WatchTx>,
}

impl<T> Clone for WatchSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
        }
    }
}

impl<T> Clone for WatchReceiver<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            handle: self.handle.clone(),
            tx_handle: self.tx_handle.clone(),
        }
    }
}

impl<T: Clone> WatchSender<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::WatchTx> {
        &self.handle
    }    pub fn send(&self, value: T) -> Result<(), watch::error::SendError<T>> {
        let source = capture_backtrace_id();
        let result = self.inner.send(value);
        if result.is_ok() {
            let _ = self
                .handle
                .mutate(|body| body.last_update_at = Some(moire_types::PTime::now()));
        }
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelSent,
            source,
        );
        record_event_with_source(event, source);
        result
    }    pub fn send_replace(&self, value: T) -> T {
        let source = capture_backtrace_id();
        let old = self.inner.send_replace(value);
        let _ = self
            .handle
            .mutate(|body| body.last_update_at = Some(moire_types::PTime::now()));
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelSent,
            source,
        );
        record_event_with_source(event, source);
        old
    }    pub fn subscribe(&self) -> WatchReceiver<T> {
        let source = capture_backtrace_id();
        let handle = EntityHandle::new(
            "watch:rx.subscribe",
            EntityBody::WatchRx(WatchRxEntity {}),
            source,
        )
        .into_typed::<moire_types::WatchRx>();
        self.handle
            .link_to_handle_with_source(&handle, EdgeKind::PairedWith, source);
        WatchReceiver {
            inner: self.inner.subscribe(),
            handle,
            tx_handle: self.handle.downgrade(),
        }
    }
}

impl<T: Clone> WatchReceiver<T> {
    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::WatchRx> {
        &self.handle
    }    pub async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        let source = capture_backtrace_id();
        let result =
            instrument_operation_on_with_source(&self.handle, self.inner.changed(), source).await;
        let event = Event::new_with_source(
            EventTarget::Entity(self.handle.id().clone()),
            EventKind::ChannelReceived,
            source,
        );
        record_event_with_source(event, source);
        result
    }

    pub fn borrow(&self) -> watch::Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn borrow_and_update(&mut self) -> watch::Ref<'_, T> {
        self.inner.borrow_and_update()
    }

    pub fn has_changed(&self) -> Result<bool, watch::error::RecvError> {
        self.inner.has_changed()
    }
}

pub fn watch<T: Clone>(name: impl Into<String>, initial: T) -> (WatchSender<T>, WatchReceiver<T>) {
    let source = capture_backtrace_id();
    let name = name.into();
    let (tx, rx) = watch::channel(initial);

    let tx_handle = EntityHandle::new(
        format!("{name}:tx"),
        EntityBody::WatchTx(WatchTxEntity {
            last_update_at: None,
        }),
        source,
    )
    .into_typed::<moire_types::WatchTx>();

    let rx_handle = EntityHandle::new(
        format!("{name}:rx"),
        EntityBody::WatchRx(WatchRxEntity {}),
        source,
    )
    .into_typed::<moire_types::WatchRx>();

    tx_handle.link_to_handle_with_source(&rx_handle, EdgeKind::PairedWith, source);

    (
        WatchSender {
            inner: tx,
            handle: tx_handle.clone(),
        },
        WatchReceiver {
            inner: rx,
            handle: rx_handle,
            tx_handle: tx_handle.downgrade(),
        },
    )
}

pub fn watch_channel<T: Clone>(
    name: impl Into<String>,
    initial: T,
) -> (WatchSender<T>, WatchReceiver<T>) {
    watch(name, initial)
}

impl<T: Clone> AsEntityRef for WatchSender<T> {
    fn as_entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }
}
