//! Task instrumentation for Tokio spawned tasks.
//!
//! When the `diagnostics` feature is enabled, wraps spawned tasks to capture
//! timing, poll events, and backtraces. When disabled, `spawn_tracked` is
//! a zero-cost wrapper around `tokio::spawn`.

use std::future::{Future, IntoFuture};

mod futures;
mod snapshot;
mod wakes;

pub use peeps_types::{GraphSnapshot, MetaBuilder};

// ── Public API (delegates to modules) ────────────────────

/// Initialize the task tracking registry. No-op without `diagnostics`.
pub fn init_task_tracking() {
    tasks::init();
    wakes::init();
    futures::init();
}

/// Spawn a tracked task with the given name.
///
/// With `diagnostics`: captures spawn backtrace and records poll events.
/// Without `diagnostics`: zero-cost wrapper around `tokio::spawn`.
#[track_caller]
pub fn spawn_tracked<F>(name: impl Into<String>, future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tasks::spawn_tracked(name, future)
}

/// Emit canonical graph nodes and edges for tasks and futures.
pub fn emit_graph(proc_key: &str) -> GraphSnapshot {
    let process_name = peeps_types::process_name().unwrap_or(proc_key);
    snapshot::emit_graph(process_name, proc_key)
}

/// Wrapper future produced by [`peepable`] or [`PeepableFutureExt::peepable`].
pub struct PeepableFuture<F> {
    inner: futures::PeepableFuture<F>,
}

impl<F> Future for PeepableFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        #[allow(unsafe_code)]
        unsafe {
            let this = self.get_unchecked_mut();
            std::pin::Pin::new_unchecked(&mut this.inner).poll(cx)
        }
    }
}

/// Mark a future as an instrumented wait on a named resource.
pub fn peepable<F>(future: F, resource: impl Into<String>) -> PeepableFuture<F::IntoFuture>
where
    F: IntoFuture,
{
    PeepableFuture {
        inner: futures::peepable(future.into_future(), resource),
    }
}

/// Mark a future as an instrumented wait with metadata.
pub fn peepable_with_meta<F, const N: usize>(
    future: F,
    resource: impl Into<String>,
    meta: MetaBuilder<'_, N>,
) -> PeepableFuture<F::IntoFuture>
where
    F: IntoFuture,
{
    PeepableFuture {
        inner: futures::peepable_with_meta(future.into_future(), resource, meta),
    }
}

/// Build a `MetaBuilder` on the stack from key-value pairs.
///
/// ```ignore
/// peep_meta!("request.id" => MetaValue::U64(42), "request.method" => MetaValue::Static("get"))
/// ```
#[macro_export]
macro_rules! peep_meta {
    ($($k:literal => $v:expr),* $(,)?) => {{
        let mut mb = $crate::MetaBuilder::<16>::new();
        $(mb.push($k, $v);)*
        mb
    }};
}

/// Wrap a future with metadata, compiling away to bare future when diagnostics are disabled.
///
/// ```ignore
/// peepable_with_meta!(
///     stream.read(&mut buf),
///     "socket.read",
///     { "request.id" => MetaValue::U64(id) }
/// ).await?;
/// ```
#[cfg(feature = "diagnostics")]
#[macro_export]
macro_rules! peepable_with_meta {
    ($future:expr, $label:literal, {$($k:literal => $v:expr),* $(,)?}) => {{
        $crate::PeepableFutureExt::peepable_with_meta(
            $future,
            $label,
            $crate::peep_meta!($($k => $v),*),
        )
    }};
}

#[cfg(not(feature = "diagnostics"))]
#[macro_export]
macro_rules! peepable_with_meta {
    ($future:expr, $label:literal, {$($k:literal => $v:expr),* $(,)?}) => {{
        $future
    }};
}

/// Wrap a future with auto-injected callsite context and optional custom metadata.
///
/// When `diagnostics` is disabled, expands to the bare future (zero cost).
///
/// ```ignore
/// // Label only (auto context injected):
/// peep!(stream.flush(), "socket.flush").await?;
///
/// // Label + custom keys:
/// peep!(stream.read(&mut buf), "socket.read", {
///     "resource.path" => path.as_str(),
///     "bytes" => buf.len(),
/// }).await?;
/// ```
#[cfg(feature = "diagnostics")]
#[macro_export]
macro_rules! peep {
    // With custom metadata keys
    ($future:expr, $label:literal, {$($k:literal => $v:expr),* $(,)?}) => {{
        let mut mb = $crate::MetaBuilder::<{
            // 6 auto context keys + user keys
            6 $(+ $crate::peep!(@count $k))*
        }>::new();
        mb.push(
            $crate::meta_key::CTX_MODULE_PATH,
            $crate::MetaValue::Static(module_path!()),
        );
        mb.push(
            $crate::meta_key::CTX_FILE,
            $crate::MetaValue::Static(file!()),
        );
        mb.push(
            $crate::meta_key::CTX_LINE,
            $crate::MetaValue::U64(line!() as u64),
        );
        mb.push(
            $crate::meta_key::CTX_CRATE_NAME,
            $crate::MetaValue::Static(env!("CARGO_PKG_NAME")),
        );
        mb.push(
            $crate::meta_key::CTX_CRATE_VERSION,
            $crate::MetaValue::Static(env!("CARGO_PKG_VERSION")),
        );
        mb.push(
            $crate::meta_key::CTX_CALLSITE,
            $crate::MetaValue::Static(concat!(
                $label, "@", file!(), ":", line!(), "::", module_path!()
            )),
        );
        $(mb.push($k, $crate::IntoMetaValue::into_meta_value($v));)*
        $crate::peepable_with_meta($future, $label, mb)
    }};
    // Label only (no custom keys)
    ($future:expr, $label:literal) => {{
        let mut mb = $crate::MetaBuilder::<6>::new();
        mb.push(
            $crate::meta_key::CTX_MODULE_PATH,
            $crate::MetaValue::Static(module_path!()),
        );
        mb.push(
            $crate::meta_key::CTX_FILE,
            $crate::MetaValue::Static(file!()),
        );
        mb.push(
            $crate::meta_key::CTX_LINE,
            $crate::MetaValue::U64(line!() as u64),
        );
        mb.push(
            $crate::meta_key::CTX_CRATE_NAME,
            $crate::MetaValue::Static(env!("CARGO_PKG_NAME")),
        );
        mb.push(
            $crate::meta_key::CTX_CRATE_VERSION,
            $crate::MetaValue::Static(env!("CARGO_PKG_VERSION")),
        );
        mb.push(
            $crate::meta_key::CTX_CALLSITE,
            $crate::MetaValue::Static(concat!(
                $label, "@", file!(), ":", line!(), "::", module_path!()
            )),
        );
        $crate::peepable_with_meta($future, $label, mb)
    }};
    // Internal: counting helper
    (@count $x:literal) => { 1usize };
}

#[cfg(not(feature = "diagnostics"))]
#[macro_export]
macro_rules! peep {
    ($future:expr, $label:literal, {$($k:literal => $v:expr),* $(,)?}) => {{
        $future
    }};
    ($future:expr, $label:literal) => {{
        $future
    }};
}

pub trait PeepableFutureExt: IntoFuture + Sized {
    fn peepable(self, resource: impl Into<String>) -> PeepableFuture<Self::IntoFuture> {
        peepable(self, resource)
    }
    fn peepable_with_meta<const N: usize>(
        self,
        resource: impl Into<String>,
        meta: MetaBuilder<'_, N>,
    ) -> PeepableFuture<Self::IntoFuture> {
        peepable_with_meta(self, resource, meta)
    }
}

impl<F: IntoFuture> PeepableFutureExt for F {}

/// Remove completed tasks from the registry. No-op without `diagnostics`.
pub fn cleanup_completed_tasks() {
    snapshot::cleanup_completed_tasks()
}
