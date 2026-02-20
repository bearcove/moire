use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Pass-through equivalent of [`tokio::task::JoinHandle`].
pub struct JoinHandle<T>(pub(crate) tokio::task::JoinHandle<T>);

impl<T> JoinHandle<T> {
    /// No-op rename — accepted for API compatibility with the enabled backend.
    pub fn named(self, _name: impl Into<String>) -> Self {
        self
    }

    pub fn abort(&self) {
        self.0.abort();
    }

    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }

    pub fn id(&self) -> tokio::task::Id {
        self.0.id()
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        unsafe { Pin::new_unchecked(&mut this.0) }.poll(cx)
    }
}
