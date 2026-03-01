use std::future::Future;
use std::io;

use moire_runtime::instrument_future;

/// Await a connect future with moire instrumentation.
pub async fn connect<F, T>(fut: F, display: &str, protocol: &str) -> io::Result<T>
where
    F: Future<Output = io::Result<T>>,
{
    let name = format!("net.connect({protocol}:{display})");
    instrument_future::<F>(name, fut, None, None).await
}

/// Await an accept future with moire instrumentation.
pub async fn accept<F, T>(fut: F, display: &str, protocol: &str) -> io::Result<T>
where
    F: Future<Output = io::Result<T>>,
{
    let name = format!("net.accept({protocol}:{display})");
    instrument_future::<F>(name, fut, None, None).await
}
