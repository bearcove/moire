use std::cell::RefCell;
use std::future::Future;

use crate::enabled::task::{IntoTaskFuture, TaskFutureExt};
use moire_runtime::{
    instrument_future_with_handle, register_current_task_scope, EntityHandle,
    FUTURE_CAUSAL_STACK,
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
    /// Creates an instrumented join set with a caller-specified name.
    pub fn new() -> Self {
                Self {
            inner: tokio::task::JoinSet::new(),
            handle: EntityHandle::new("joinset", FutureEntity {}),
        }
    }

    /// Creates an instrumented join set equivalent to [`tokio::task::JoinSet::new`].
    pub fn named(name: impl Into<String>) -> Self {
        let name = name.into();
        let handle = EntityHandle::new(format!("joinset.{name}"), FutureEntity {});
        Self {
            inner: tokio::task::JoinSet::new(),
            handle,
        }
    }

    /// Spawns a future into the set, matching [`tokio::task::JoinSet::spawn`].
    pub fn spawn<TF>(&mut self, task: TF)
    where
        TF: IntoTaskFuture<T>,
    {
        let joinset_handle = self.handle.clone();
        let task = task.into_task_future();
        let name = task
            .name
            .unwrap_or_else(|| String::from("joinset.task"));
        let on = task.on.or(Some(joinset_handle.entity_ref()));
        let task_handle = EntityHandle::new(name, FutureEntity {});
        self.inner.spawn(
            FUTURE_CAUSAL_STACK.scope(RefCell::new(Vec::new()), async move {
                let _task_scope = register_current_task_scope("joinset.spawn");
                instrument_future_with_handle(task_handle, task.future, on, None).await
            }),
        );
    }

    /// Spawns a named future into the set.
    pub fn spawn_named<F>(&mut self, name: impl Into<String>, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        self.spawn(future.named(name));
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
