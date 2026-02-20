use std::sync::Arc;

pub use tokio::sync::{AcquireError, OwnedSemaphorePermit, SemaphorePermit, TryAcquireError};

/// Pass-through `tokio::sync::Semaphore` wrapper, accepting a name parameter for API parity.
#[derive(Clone)]
pub struct Semaphore(Arc<tokio::sync::Semaphore>);

impl Semaphore {
    pub fn new(_name: impl Into<String>, permits: usize) -> Self {
        Self(Arc::new(tokio::sync::Semaphore::new(permits)))
    }

    pub fn available_permits(&self) -> usize {
        self.0.available_permits()
    }

    pub fn close(&self) {
        self.0.close()
    }

    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    pub fn add_permits(&self, n: usize) {
        self.0.add_permits(n)
    }

    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, AcquireError> {
        self.0.acquire().await
    }

    pub async fn acquire_many(&self, n: u32) -> Result<SemaphorePermit<'_>, AcquireError> {
        self.0.acquire_many(n).await
    }

    pub async fn acquire_owned(&self) -> Result<OwnedSemaphorePermit, AcquireError> {
        Arc::clone(&self.0).acquire_owned().await
    }

    pub async fn acquire_many_owned(&self, n: u32) -> Result<OwnedSemaphorePermit, AcquireError> {
        Arc::clone(&self.0).acquire_many_owned(n).await
    }

    pub fn try_acquire(&self) -> Result<SemaphorePermit<'_>, TryAcquireError> {
        self.0.try_acquire()
    }

    pub fn try_acquire_many(&self, n: u32) -> Result<SemaphorePermit<'_>, TryAcquireError> {
        self.0.try_acquire_many(n)
    }

    pub fn try_acquire_owned(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.0).try_acquire_owned()
    }

    pub fn try_acquire_many_owned(&self, n: u32) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.0).try_acquire_many_owned(n)
    }
}
