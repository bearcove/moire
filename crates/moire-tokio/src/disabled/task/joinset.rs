use std::fmt;
use std::future::Future;

/// Pass-through equivalent of [`tokio::task::JoinSet`].
pub struct JoinSet<T>(tokio::task::JoinSet<T>);

impl<T> JoinSet<T>
where
    T: Send + 'static,
{
    pub fn new() -> Self {
        Self(tokio::task::JoinSet::new())
    }

    /// Accepted for API compatibility with the enabled backend; name is ignored.
    pub fn named(_name: impl Into<String>) -> Self {
        Self::new()
    }

    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        self.0.spawn(future);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn abort_all(&mut self) {
        self.0.abort_all();
    }

    pub fn join_next(
        &mut self,
    ) -> impl Future<Output = Option<Result<T, tokio::task::JoinError>>> + '_ {
        self.0.join_next()
    }
}

impl<T> Default for JoinSet<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for JoinSet<T>
where
    T: Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinSet")
            .field("len", &self.len())
            .field("is_empty", &self.is_empty())
            .finish()
    }
}
