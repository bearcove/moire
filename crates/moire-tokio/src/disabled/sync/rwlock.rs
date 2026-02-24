use std::fmt;

/// Pass-through `tokio::sync::RwLock` wrapper, accepting a name parameter for API parity.
pub struct RwLock<T>(tokio::sync::RwLock<T>);

pub use tokio::sync::{RwLockReadGuard, RwLockWriteGuard, TryLockError as AsyncRwLockTryLockError};

/// Pass-through `parking_lot::RwLock` wrapper, accepting a name parameter for API parity.
pub struct SyncRwLock<T>(parking_lot::RwLock<T>);

pub use parking_lot::{
    RwLockReadGuard as SyncRwLockReadGuard, RwLockWriteGuard as SyncRwLockWriteGuard,
};

impl<T> RwLock<T> {
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(tokio::sync::RwLock::new(value))
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0.read().await
    }

    pub async fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write().await
    }

    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>, AsyncRwLockTryLockError> {
        self.0.try_read()
    }

    pub fn try_write(&self) -> Result<RwLockWriteGuard<'_, T>, AsyncRwLockTryLockError> {
        self.0.try_write()
    }
}

impl<T> SyncRwLock<T> {
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(parking_lot::RwLock::new(value))
    }

    pub fn read(&self) -> SyncRwLockReadGuard<'_, T> {
        self.0.read()
    }

    pub fn write(&self) -> SyncRwLockWriteGuard<'_, T> {
        self.0.write()
    }

    pub fn try_read(&self) -> Option<SyncRwLockReadGuard<'_, T>> {
        self.0.try_read()
    }

    pub fn try_write(&self) -> Option<SyncRwLockWriteGuard<'_, T>> {
        self.0.try_write()
    }
}

impl<T: fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: fmt::Debug> fmt::Debug for SyncRwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
