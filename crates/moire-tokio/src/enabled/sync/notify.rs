// r[impl api.notify]
use moire_types::NotifyEntity;
use std::sync::Arc;

use moire_runtime::{instrument_operation_on, EntityHandle};

/// Instrumented version of [`tokio::sync::Notify`].
#[derive(Clone)]
pub struct Notify {
    inner: Arc<tokio::sync::Notify>,
    handle: EntityHandle<NotifyEntity>,
}

impl Notify {
    /// Creates a new instrumented notify, matching [`tokio::sync::Notify::new`].
    pub fn new(name: impl Into<String>) -> Self {
        let handle = EntityHandle::new(name, NotifyEntity { waiter_count: 0 });
        Self {
            inner: Arc::new(tokio::sync::Notify::new()),
            handle,
        }
    }
    /// Waits for a notification, matching [`tokio::sync::Notify::notified`].
    pub async fn notified(&self) {
                let _ = self
            .handle
            .mutate(|body| body.waiter_count = body.waiter_count.saturating_add(1));

        instrument_operation_on(&self.handle, self.inner.notified()).await;

        let _ = self
            .handle
            .mutate(|body| body.waiter_count = body.waiter_count.saturating_sub(1));
    }

    /// Notifies one waiter, matching [`tokio::sync::Notify::notify_one`].
    pub fn notify_one(&self) {
        self.inner.notify_one();
    }

    /// Notifies all waiters, matching [`tokio::sync::Notify::notify_waiters`].
    pub fn notify_waiters(&self) {
        self.inner.notify_waiters();
    }
}
