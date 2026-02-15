// ── Zero-cost stubs (no diagnostics) ────────────────────

// ── mpsc bounded ────────────────────────────────────────

pub(crate) struct Sender<T>(tokio::sync::mpsc::Sender<T>);

impl<T> Clone for Sender<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Sender<T> {
    #[inline]
    pub(crate) async fn send(
        &self,
        value: T,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<T>> {
        self.0.send(value).await
    }

    #[inline]
    pub(crate) fn try_send(
        &self,
        value: T,
    ) -> Result<(), tokio::sync::mpsc::error::TrySendError<T>> {
        self.0.try_send(value)
    }

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.0.capacity()
    }

    #[inline]
    pub(crate) fn max_capacity(&self) -> usize {
        self.0.max_capacity()
    }
}

pub(crate) struct Receiver<T>(tokio::sync::mpsc::Receiver<T>);

impl<T> Receiver<T> {
    #[inline]
    pub(crate) async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }

    #[inline]
    pub(crate) fn try_recv(&mut self) -> Result<T, tokio::sync::mpsc::error::TryRecvError> {
        self.0.try_recv()
    }

    #[inline]
    pub(crate) fn close(&mut self) {
        self.0.close()
    }
}

#[inline]
pub(crate) fn channel<T>(_name: impl Into<String>, buffer: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::channel(buffer);
    (Sender(tx), Receiver(rx))
}

// ── mpsc unbounded ──────────────────────────────────────

pub(crate) struct UnboundedSender<T>(tokio::sync::mpsc::UnboundedSender<T>);

impl<T> Clone for UnboundedSender<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> UnboundedSender<T> {
    #[inline]
    pub(crate) fn send(
        &self,
        value: T,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<T>> {
        self.0.send(value)
    }

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

pub(crate) struct UnboundedReceiver<T>(tokio::sync::mpsc::UnboundedReceiver<T>);

impl<T> UnboundedReceiver<T> {
    #[inline]
    pub(crate) async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }

    #[inline]
    pub(crate) fn try_recv(&mut self) -> Result<T, tokio::sync::mpsc::error::TryRecvError> {
        self.0.try_recv()
    }

    #[inline]
    pub(crate) fn close(&mut self) {
        self.0.close()
    }
}

#[inline]
pub(crate) fn unbounded_channel<T>(
    _name: impl Into<String>,
) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (UnboundedSender(tx), UnboundedReceiver(rx))
}

// ── oneshot ─────────────────────────────────────────────

pub(crate) struct OneshotSender<T>(tokio::sync::oneshot::Sender<T>);

impl<T> OneshotSender<T> {
    #[inline]
    pub(crate) fn send(self, value: T) -> Result<(), T> {
        self.0.send(value)
    }

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

pub(crate) struct OneshotReceiver<T>(tokio::sync::oneshot::Receiver<T>);

impl<T> OneshotReceiver<T> {
    #[inline]
    pub(crate) async fn recv(self) -> Result<T, tokio::sync::oneshot::error::RecvError> {
        self.0.await
    }

    #[inline]
    pub(crate) fn try_recv(&mut self) -> Result<T, tokio::sync::oneshot::error::TryRecvError> {
        self.0.try_recv()
    }
}

#[inline]
pub(crate) fn oneshot_channel<T>(
    _name: impl Into<String>,
) -> (OneshotSender<T>, OneshotReceiver<T>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    (OneshotSender(tx), OneshotReceiver(rx))
}

// ── watch ───────────────────────────────────────────────

pub(crate) struct WatchSender<T>(tokio::sync::watch::Sender<T>);

impl<T> WatchSender<T> {
    #[inline]
    pub(crate) fn send(
        &self,
        value: T,
    ) -> Result<(), tokio::sync::watch::error::SendError<T>> {
        self.0.send(value)
    }

    #[inline]
    pub(crate) fn send_modify<F: FnOnce(&mut T)>(&self, modify: F) {
        self.0.send_modify(modify)
    }

    #[inline]
    pub(crate) fn send_if_modified<F: FnOnce(&mut T) -> bool>(&self, modify: F) -> bool {
        self.0.send_if_modified(modify)
    }

    #[inline]
    pub(crate) fn borrow(&self) -> tokio::sync::watch::Ref<'_, T> {
        self.0.borrow()
    }

    #[inline]
    pub(crate) fn receiver_count(&self) -> usize {
        self.0.receiver_count()
    }

    #[inline]
    pub(crate) fn subscribe(&self) -> WatchReceiver<T> {
        WatchReceiver(self.0.subscribe())
    }

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

pub(crate) struct WatchReceiver<T>(tokio::sync::watch::Receiver<T>);

impl<T> Clone for WatchReceiver<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> WatchReceiver<T> {
    #[inline]
    pub(crate) async fn changed(
        &mut self,
    ) -> Result<(), tokio::sync::watch::error::RecvError> {
        self.0.changed().await
    }

    #[inline]
    pub(crate) fn borrow(&self) -> tokio::sync::watch::Ref<'_, T> {
        self.0.borrow()
    }

    #[inline]
    pub(crate) fn borrow_and_update(&mut self) -> tokio::sync::watch::Ref<'_, T> {
        self.0.borrow_and_update()
    }

    #[inline]
    pub(crate) fn has_changed(&self) -> Result<bool, tokio::sync::watch::error::RecvError> {
        self.0.has_changed()
    }
}

#[inline]
pub(crate) fn watch_channel<T>(
    _name: impl Into<String>,
    init: T,
) -> (WatchSender<T>, WatchReceiver<T>) {
    let (tx, rx) = tokio::sync::watch::channel(init);
    (WatchSender(tx), WatchReceiver(rx))
}

// ── Semaphore ───────────────────────────────────────────

pub(crate) struct DiagnosticSemaphore(std::sync::Arc<tokio::sync::Semaphore>);

impl Clone for DiagnosticSemaphore {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl DiagnosticSemaphore {
    #[inline]
    pub(crate) fn new(_name: impl Into<String>, permits: usize) -> Self {
        Self(std::sync::Arc::new(tokio::sync::Semaphore::new(permits)))
    }

    #[inline]
    pub(crate) fn available_permits(&self) -> usize {
        self.0.available_permits()
    }

    #[inline]
    pub(crate) fn close(&self) {
        self.0.close()
    }

    #[inline]
    pub(crate) fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    #[inline]
    pub(crate) fn add_permits(&self, n: usize) {
        self.0.add_permits(n)
    }

    #[inline]
    pub(crate) async fn acquire(
        &self,
    ) -> Result<tokio::sync::SemaphorePermit<'_>, tokio::sync::AcquireError> {
        self.0.acquire().await
    }

    #[inline]
    pub(crate) async fn acquire_many(
        &self,
        n: u32,
    ) -> Result<tokio::sync::SemaphorePermit<'_>, tokio::sync::AcquireError> {
        self.0.acquire_many(n).await
    }

    #[inline]
    pub(crate) async fn acquire_owned(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::AcquireError> {
        self.0.clone().acquire_owned().await
    }

    #[inline]
    pub(crate) async fn acquire_many_owned(
        &self,
        n: u32,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::AcquireError> {
        self.0.clone().acquire_many_owned(n).await
    }

    #[inline]
    pub(crate) fn try_acquire(
        &self,
    ) -> Result<tokio::sync::SemaphorePermit<'_>, tokio::sync::TryAcquireError> {
        self.0.try_acquire()
    }

    #[inline]
    pub(crate) fn try_acquire_many(
        &self,
        n: u32,
    ) -> Result<tokio::sync::SemaphorePermit<'_>, tokio::sync::TryAcquireError> {
        self.0.try_acquire_many(n)
    }

    #[inline]
    pub(crate) fn try_acquire_owned(
        &self,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::TryAcquireError> {
        self.0.clone().try_acquire_owned()
    }

    #[inline]
    pub(crate) fn try_acquire_many_owned(
        &self,
        n: u32,
    ) -> Result<tokio::sync::OwnedSemaphorePermit, tokio::sync::TryAcquireError> {
        self.0.clone().try_acquire_many_owned(n)
    }
}

// ── OnceCell ────────────────────────────────────────────

pub(crate) struct OnceCell<T>(tokio::sync::OnceCell<T>);

impl<T> OnceCell<T> {
    #[inline]
    pub(crate) fn new(_name: impl Into<String>) -> Self {
        Self(tokio::sync::OnceCell::new())
    }

    #[inline]
    pub(crate) fn get(&self) -> Option<&T> {
        self.0.get()
    }

    #[inline]
    pub(crate) fn initialized(&self) -> bool {
        self.0.initialized()
    }

    #[inline]
    pub(crate) async fn get_or_init<F, Fut>(&self, f: F) -> &T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        self.0.get_or_init(f).await
    }

    #[inline]
    pub(crate) async fn get_or_try_init<F, Fut, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        self.0.get_or_try_init(f).await
    }

    #[inline]
    pub(crate) fn set(&self, value: T) -> Result<(), T> {
        self.0.set(value).map_err(|e| match e {
            tokio::sync::SetError::AlreadyInitializedError(v) => v,
            tokio::sync::SetError::InitializingError(v) => v,
        })
    }
}

// ── Graph emission (no-op) ──────────────────────────────

#[inline(always)]
pub(crate) fn emit_sync_graph(_graph: &mut peeps_types::GraphSnapshot) {}
