use peeps_types::GraphSnapshot;

#[inline(always)]
pub(crate) fn emit_into_graph(_graph: &mut GraphSnapshot) {}

pub struct DiagnosticRwLock<T>(parking_lot::RwLock<T>);

impl<T> DiagnosticRwLock<T> {
    #[inline]
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(parking_lot::RwLock::new(value))
    }

    #[inline]
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.0.read()
    }

    #[inline]
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.0.write()
    }
}

pub struct DiagnosticMutex<T>(parking_lot::Mutex<T>);

impl<T> DiagnosticMutex<T> {
    #[inline]
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(parking_lot::Mutex::new(value))
    }

    #[inline]
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, T> {
        self.0.lock()
    }
}

pub struct DiagnosticAsyncRwLock<T>(tokio::sync::RwLock<T>);

impl<T> DiagnosticAsyncRwLock<T> {
    #[inline]
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(tokio::sync::RwLock::new(value))
    }

    #[inline]
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, T> {
        self.0.read().await
    }

    #[inline]
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, T> {
        self.0.write().await
    }

    #[inline]
    pub fn try_read(
        &self,
    ) -> Result<tokio::sync::RwLockReadGuard<'_, T>, tokio::sync::TryLockError> {
        self.0.try_read()
    }

    #[inline]
    pub fn try_write(
        &self,
    ) -> Result<tokio::sync::RwLockWriteGuard<'_, T>, tokio::sync::TryLockError> {
        self.0.try_write()
    }
}

pub struct DiagnosticAsyncMutex<T>(tokio::sync::Mutex<T>);

impl<T> DiagnosticAsyncMutex<T> {
    #[inline]
    pub fn new(_name: &'static str, value: T) -> Self {
        Self(tokio::sync::Mutex::new(value))
    }

    #[inline]
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, T> {
        self.0.lock().await
    }

    #[inline]
    pub fn try_lock(&self) -> Result<tokio::sync::MutexGuard<'_, T>, tokio::sync::TryLockError> {
        self.0.try_lock()
    }
}
