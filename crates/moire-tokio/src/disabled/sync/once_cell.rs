use std::future::Future;

/// Pass-through `tokio::sync::OnceCell` wrapper, accepting a name parameter for API parity.
pub struct OnceCell<T>(tokio::sync::OnceCell<T>);

impl<T> OnceCell<T> {
    pub fn new(_name: impl Into<String>) -> Self {
        Self(tokio::sync::OnceCell::new())
    }

    pub fn get(&self) -> Option<&T> {
        self.0.get()
    }

    pub fn initialized(&self) -> bool {
        self.0.initialized()
    }

    pub async fn get_or_init<'a, F, Fut>(&'a self, f: F) -> &'a T
    where
        F: FnOnce() -> Fut + 'a,
        Fut: Future<Output = T> + 'a,
    {
        self.0.get_or_init(f).await
    }

    pub async fn get_or_try_init<'a, F, Fut, E>(&'a self, f: F) -> Result<&'a T, E>
    where
        F: FnOnce() -> Fut + 'a,
        Fut: Future<Output = Result<T, E>> + 'a,
    {
        self.0.get_or_try_init(f).await
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        self.0.set(value).map_err(|e| match e {
            tokio::sync::SetError::AlreadyInitializedError(v) => v,
            tokio::sync::SetError::InitializingError(v) => v,
        })
    }
}
