// r[impl api.time]
use std::future::Future;
use std::time::Duration;

use moire_types::EntityBody;
use moire_runtime::{instrument_future, EntityHandle};


/// Instrumented equivalent of [`tokio::time::sleep`].
pub fn sleep(duration: Duration) -> impl Future<Output = ()> {
        let handle = EntityHandle::new_untyped(
        "time.sleep",
        EntityBody::Future(moire_types::FutureEntity {}), 
    );

    instrument_future(
        "time.sleep",
        tokio::time::sleep(duration), 
        Some(handle.entity_ref()),
        None,
    )
}

/// Instrumented equivalent of [`tokio::time::Interval`].
pub struct Interval {
    inner: tokio::time::Interval,
    handle: EntityHandle,
}

impl Interval {
    /// Creates an instrumented interval, equivalent to [`tokio::time::Interval::new`].
    pub fn new(period: Duration) -> Self {
                Self {
            inner: tokio::time::interval(period),
            handle: EntityHandle::new_untyped(
                "time.interval",
                EntityBody::Future(moire_types::FutureEntity {}), 
            ),
        }
    }

    /// Waits for the next tick, equivalent to [`tokio::time::Interval::tick`].
    pub fn tick(&mut self) -> impl Future<Output = tokio::time::Instant> + '_ {
                instrument_future(
            "time.interval.tick",
            self.inner.tick(), 
            Some(self.handle.entity_ref()),
            None,
        )
    }
}

/// Creates an instrumented interval, matching [`tokio::time::interval`].
pub fn interval(period: Duration) -> Interval {
    Interval::new(period)
}
