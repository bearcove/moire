use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use moire_runtime::EntityHandle;
use moire_types::FutureEntity;

/// Instrumented equivalent of [`tokio::task::JoinHandle`].
pub struct JoinHandle<T> {
    inner: tokio::task::JoinHandle<T>,
    handle: EntityHandle<FutureEntity>,
}

impl<T> JoinHandle<T> {
    pub(crate) fn new(
        inner: tokio::task::JoinHandle<T>,
        handle: EntityHandle<FutureEntity>,
    ) -> Self {
        Self { inner, handle }
    }

    /// Renames the underlying task entity.
    pub fn named(self, name: impl Into<String>) -> Self {
        let _ = self.handle.rename(name);
        self
    }

    /// Aborts the associated task.
    pub fn abort(&self) {
        self.inner.abort();
    }

    /// Returns `true` if this task has finished.
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// Returns the task identifier.
    pub fn id(&self) -> tokio::task::Id {
        self.inner.id()
    }

    /// Returns the tracked entity reference for this task.
    #[doc(hidden)]
    pub fn entity_handle(&self) -> &EntityHandle<FutureEntity> {
        &self.handle
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.inner) }.poll(cx)
    }
}

impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinHandle")
            .field("id", &self.id())
            .field("is_finished", &self.is_finished())
            .finish()
    }
}
