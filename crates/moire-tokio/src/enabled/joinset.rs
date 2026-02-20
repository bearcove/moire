use std::cell::RefCell;
use std::future::Future;

use super::process::JoinSet;
use super::capture_backtrace_id;
use moire_runtime::{
    instrument_future, register_current_task_scope, EntityHandle, FUTURE_CAUSAL_STACK,
};

impl<T> JoinSet<T>
where
    T: Send + 'static,
{    pub fn named(name: impl Into<String>) -> Self {
        let source = capture_backtrace_id();
        let name = name.into();
        let handle = EntityHandle::new(
            format!("joinset.{name}"),
            moire_types::EntityBody::Future(moire_types::FutureEntity {}),
            source,
        );
        Self {
            inner: tokio::task::JoinSet::new(),
            handle,
        }
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self::named(name)
    }    pub fn spawn<F>(&mut self, label: &'static str, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        let source = capture_backtrace_id();
        let joinset_handle = self.handle.clone();
        self.inner.spawn(
            FUTURE_CAUSAL_STACK.scope(RefCell::new(Vec::new()), async move {
                let _task_scope = register_current_task_scope(label, source);
                instrument_future(
                    label,
                    future,
                    source,
                    Some(joinset_handle.entity_ref()),
                    None,
                )
                .await
            }),
        );
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn abort_all(&mut self) {
        self.inner.abort_all();
    }    pub fn join_next(&mut self) -> impl Future<Output = Option<Result<T, tokio::task::JoinError>>> + '_ {
        let source = capture_backtrace_id();
        let handle = self.handle.clone();
        let fut = self.inner.join_next();
        instrument_future(
            "joinset.join_next",
            fut,
            source,
            Some(handle.entity_ref()),
            None,
        )
    }
}
