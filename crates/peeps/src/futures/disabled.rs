use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::task::{Context, Poll};

use peeps_types::GraphSnapshot;

// ── PeepableFuture (zero-cost wrapper) ───────────────────

pub struct PeepableFuture<F> {
    inner: F,
}

impl<F> Future for PeepableFuture<F>
where
    F: Future,
{
    type Output = F::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we never move `inner` after pinning `Self`.
        #[allow(unsafe_code)]
        unsafe {
            let this = self.get_unchecked_mut();
            Pin::new_unchecked(&mut this.inner).poll(cx)
        }
    }
}

// ── Construction ─────────────────────────────────────────

#[inline]
pub fn peepable<F>(future: F, _resource: impl Into<String>) -> PeepableFuture<F::IntoFuture>
where
    F: IntoFuture,
{
    PeepableFuture {
        inner: future.into_future(),
    }
}

#[inline]
pub fn peepable_with_meta<F, const N: usize>(
    future: F,
    _resource: impl Into<String>,
    _meta: peeps_types::MetaBuilder<'_, N>,
) -> PeepableFuture<F::IntoFuture>
where
    F: IntoFuture,
{
    PeepableFuture {
        inner: future.into_future(),
    }
}

// ── spawn_tracked ────────────────────────────────────────

#[inline]
pub fn spawn_tracked<F>(_name: impl Into<String>, future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future)
}

// ── Graph emission (no-op) ───────────────────────────────

#[inline(always)]
pub(crate) fn emit_into_graph(_graph: &mut GraphSnapshot) {}
