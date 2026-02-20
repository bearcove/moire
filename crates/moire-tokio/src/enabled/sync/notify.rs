// r[impl api.notify]
use moire_types::{EntityBody, NotifyEntity};
use std::sync::Arc;

use super::super::capture_backtrace_id;
use moire_runtime::{instrument_operation_on_with_source, EntityHandle};

#[derive(Clone)]
pub struct Notify {
    inner: Arc<tokio::sync::Notify>,
    handle: EntityHandle<moire_types::Notify>,
}

impl Notify {
    pub fn new(name: impl Into<String>) -> Self {
        let source = capture_backtrace_id();
        let handle = EntityHandle::new(
            name.into(),
            EntityBody::Notify(NotifyEntity { waiter_count: 0 }),
            source,
        )
        .into_typed::<moire_types::Notify>();
        Self {
            inner: Arc::new(tokio::sync::Notify::new()),
            handle,
        }
    }
    pub async fn notified(&self) {
        let source = capture_backtrace_id();
        let _ = self
            .handle
            .mutate(|body| body.waiter_count = body.waiter_count.saturating_add(1));

        instrument_operation_on_with_source(&self.handle, self.inner.notified(), source).await;

        let _ = self
            .handle
            .mutate(|body| body.waiter_count = body.waiter_count.saturating_sub(1));
    }

    pub fn notify_one(&self) {
        self.inner.notify_one();
    }

    pub fn notify_waiters(&self) {
        self.inner.notify_waiters();
    }
}
