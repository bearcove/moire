/// Pass-through `parking_lot::RwLock` wrapper, accepting a name parameter for API parity.
pub struct RwLock<T>(parking_lot::RwLock<T>);

pub use parking_lot::{RwLockReadGuard, RwLockWriteGuard};

impl<T> RwLock<T> {
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(parking_lot::RwLock::new(value))
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0.read()
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write()
    }

    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        self.0.try_read()
    }

    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        self.0.try_write()
    }
}
