use std::fmt;
use std::ops::Deref;

/// Pass-through `tokio::sync::Mutex` wrapper, accepting a name parameter for API parity.
pub struct Mutex<T>(tokio::sync::Mutex<T>);

pub use tokio::sync::MutexGuard;

/// Pass-through `parking_lot::Mutex` wrapper, accepting a name parameter for API parity.
pub struct SyncMutex<T>(parking_lot::Mutex<T>);

pub use parking_lot::MutexGuard as SyncMutexGuard;

impl<T> Mutex<T> {
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(tokio::sync::Mutex::new(value))
    }

    pub async fn lock(&self) -> MutexGuard<'_, T> {
        self.0.lock().await
    }

    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>, tokio::sync::TryLockError> {
        self.0.try_lock()
    }
}

impl<T> SyncMutex<T> {
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(parking_lot::Mutex::new(value))
    }

    pub fn lock(&self) -> SyncMutexGuard<'_, T> {
        self.0.lock()
    }

    pub fn try_lock(&self) -> Option<SyncMutexGuard<'_, T>> {
        self.0.try_lock()
    }
}

impl<T> Deref for SyncMutex<T> {
    type Target = parking_lot::Mutex<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for SyncMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
