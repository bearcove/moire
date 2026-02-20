//! Instrumented task spawning and management, mirroring [`tokio::task`].
//!
//! This module mirrors the structure of `tokio::task` and can be used as a
//! drop-in replacement. Spawned tasks and join sets are registered as named
//! entities in the Moiré runtime graph, so the dashboard can show the full
//! dependency graph between tasks and what each one is currently waiting on.
//!
//! # Available items
//!
//! | Item | Tokio equivalent |
//! |---|---|
//! | [`JoinSet`] | `tokio::task::JoinSet` |
//! | [`JoinHandle`] | `tokio::task::JoinHandle` |
//! | [`spawn`] | `tokio::task::spawn` |
//! | [`spawn_blocking`] | `tokio::task::spawn_blocking` |

pub mod joinset;
pub mod join_handle;

pub use self::joinset::*;
pub use self::join_handle::*;

use std::cell::RefCell;
use std::future::Future;

use moire_runtime::{
    instrument_future_with_handle, register_current_task_scope, AsEntityRef, EntityHandle,
    EntityRef, FUTURE_CAUSAL_STACK,
};
use moire_types::FutureEntity;

/// Future wrapper that carries task instrumentation metadata.
pub struct TaskFuture<F> {
    pub(crate) future: F,
    pub(crate) name: Option<String>,
    pub(crate) on: Option<EntityRef>,
}

impl<F> TaskFuture<F> {
    fn new(future: F) -> Self {
        Self {
            future,
            name: None,
            on: None,
        }
    }

    /// Sets the future/task name.
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the target entity this future is waiting on.
    pub fn on(mut self, target: impl AsEntityRef) -> Self {
        self.on = Some(target.as_entity_ref());
        self
    }
}

/// Extension trait for adding instrumentation metadata to futures.
pub trait TaskFutureExt: Future + Sized {
    /// Sets the future/task name.
    fn named(self, name: impl Into<String>) -> TaskFuture<Self> {
        TaskFuture::new(self).named(name)
    }

    /// Sets the target entity this future is waiting on.
    fn on(self, target: impl AsEntityRef) -> TaskFuture<Self> {
        TaskFuture::new(self).on(target)
    }
}

impl<F> TaskFutureExt for F where F: Future + Sized {}

pub trait IntoTaskFuture<T> {
    type Fut: Future<Output = T> + Send + 'static;
    fn into_task_future(self) -> TaskFuture<Self::Fut>;
}

impl<T, F> IntoTaskFuture<T> for F
where
    F: Future<Output = T> + Send + 'static,
{
    type Fut = F;

    fn into_task_future(self) -> TaskFuture<Self::Fut> {
        TaskFuture::new(self)
    }
}

impl<T, F> IntoTaskFuture<T> for TaskFuture<F>
where
    F: Future<Output = T> + Send + 'static,
{
    type Fut = F;

    fn into_task_future(self) -> TaskFuture<Self::Fut> {
        self
    }
}

/// Spawns a task, equivalent to [`tokio::task::spawn`].
pub fn spawn<T, TF>(task: TF) -> JoinHandle<T>
where
    T: Send + 'static,
    TF: IntoTaskFuture<T>,
{
    let TaskFuture { future, name, on } = task.into_task_future();
    let handle = EntityHandle::new(name.unwrap_or_else(|| String::from("task.spawn")), FutureEntity {});
    let future_handle = handle.clone();
    let fut = FUTURE_CAUSAL_STACK.scope(RefCell::new(Vec::new()), async move {
        let _task_scope = register_current_task_scope("spawn");
        instrument_future_with_handle(future_handle, future, on, None).await
    });
    JoinHandle::new(tokio::spawn(fut), handle)
}

/// Spawns a blocking task, equivalent to [`tokio::task::spawn_blocking`].
pub fn spawn_blocking<T, F>(f: F) -> JoinHandle<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let handle = EntityHandle::new("task.spawn_blocking", FutureEntity {});
    let inner = tokio::task::spawn_blocking(move || {
        let _task_scope = register_current_task_scope("spawn_blocking");
        f()
    });
    JoinHandle::new(inner, handle)
}
