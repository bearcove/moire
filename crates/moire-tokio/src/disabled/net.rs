use std::future::Future;
use std::io;

/// Await a connect future (no-op instrumentation).
pub async fn connect<F, T>(fut: F, _display: &str, _protocol: &str) -> io::Result<T>
where
    F: Future<Output = io::Result<T>>,
{
    fut.await
}

/// Await an accept future (no-op instrumentation).
pub async fn accept<F, T>(fut: F, _display: &str, _protocol: &str) -> io::Result<T>
where
    F: Future<Output = io::Result<T>>,
{
    fut.await
}
