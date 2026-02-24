use std::cell::RefCell;
use std::fmt;
use std::future::Future;

use moire_runtime::{
    EntityHandle, FUTURE_CAUSAL_STACK, instrument_future_with_handle, register_current_task_scope,
};
use moire_types::FutureEntity;

/// Instrumented equivalent of [`tokio::task::JoinSet`], used to track joined task sets.
pub struct JoinSet<T> {
    pub(super) inner: tokio::task::JoinSet<T>,
    pub(super) handle: EntityHandle<FutureEntity>,
}

// r[impl api.joinset]
impl<T> JoinSet<T>
where
    T: Send + 'static,
{
    /// Creates an instrumented join set, equivalent to [`tokio::task::JoinSet::new`].
    pub fn new() -> Self {
        Self {
            inner: tokio::task::JoinSet::new(),
            handle: EntityHandle::new("joinset", FutureEntity {}),
        }
    }

    /// Creates a named instrumented join set.
    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        let handle = EntityHandle::new(format!("joinset.{name}"), FutureEntity {});
        Self {
            inner: tokio::task::JoinSet::new(),
            handle,
        }
    }

    /// Spawns a future into the set, matching [`tokio::task::JoinSet::spawn`].
    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        let joinset_handle = self.handle.clone();
        let task_handle = EntityHandle::new("joinset.task", FutureEntity {});
        self.inner.spawn(
            FUTURE_CAUSAL_STACK.scope(RefCell::new(Vec::new()), async move {
                let _task_scope = register_current_task_scope("joinset.spawn");
                instrument_future_with_handle(
                    task_handle,
                    future,
                    Some(joinset_handle.entity_ref()),
                    None,
                )
                .await
            }),
        );
    }

    /// Returns whether the set is empty, matching [`tokio::task::JoinSet::is_empty`].
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of tasks still tracked, like [`tokio::task::JoinSet::len`].
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Aborts all in-flight tasks, equivalent to [`tokio::task::JoinSet::abort_all`].
    pub fn abort_all(&mut self) {
        self.inner.abort_all();
    }

    /// Waits for one task to complete, matching [`tokio::task::JoinSet::join_next`].
    pub fn join_next(
        &mut self,
    ) -> impl Future<Output = Option<Result<T, tokio::task::JoinError>>> + '_ {
        let handle = self.handle.clone();
        let fut_handle = EntityHandle::new("joinset.join_next", FutureEntity {});
        let fut = self.inner.join_next();
        instrument_future_with_handle(fut_handle, fut, Some(handle.entity_ref()), None)
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
