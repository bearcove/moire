use std::future::{Future, IntoFuture};

/// Wrap a connect future with network readiness instrumentation.
/// When diagnostics are disabled, this is a zero-cost pass-through.
#[inline]
pub fn connect<F: IntoFuture>(
    future: F,
    _endpoint: &str,
    _transport: &str,
) -> F::IntoFuture {
    future.into_future()
}

/// Wrap an accept future with network readiness instrumentation.
/// When diagnostics are disabled, this is a zero-cost pass-through.
#[inline]
pub fn accept<F: IntoFuture>(
    future: F,
    _endpoint: &str,
    _transport: &str,
) -> F::IntoFuture {
    future.into_future()
}

/// Wrap a readable readiness wait with network instrumentation.
/// When diagnostics are disabled, this is a zero-cost pass-through.
#[inline]
pub fn readable<F: IntoFuture>(
    future: F,
    _endpoint: &str,
    _transport: &str,
) -> F::IntoFuture {
    future.into_future()
}

/// Wrap a writable readiness wait with network instrumentation.
/// When diagnostics are disabled, this is a zero-cost pass-through.
#[inline]
pub fn writable<F: IntoFuture>(
    future: F,
    _endpoint: &str,
    _transport: &str,
) -> F::IntoFuture {
    future.into_future()
}
