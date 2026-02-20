pub mod channel_full_stall;
pub mod mutex_lock_order_inversion;
pub mod oneshot_sender_lost_in_map;
#[cfg(feature = "roam")]
pub mod roam_rpc_stuck_request;
#[cfg(feature = "roam")]
pub mod roam_rust_swift_stuck_request;
pub mod semaphore_starvation;

use std::future::Future;

const EXAMPLES_SOURCE_LEFT: moire::SourceLeft =
    moire::SourceLeft::new(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"));

#[track_caller]
pub(crate) fn spawn_tracked<F>(
    name: impl Into<String>,
    fut: F,
) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    moire::spawn_tracked(
        name,
        fut,
        EXAMPLES_SOURCE_LEFT.join(moire::SourceRight::caller()).into(),
    )
}
