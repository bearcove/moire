// r[impl api.time]
//! Instrumented time utilities, mirroring [`tokio::time`].
//!
//! This module mirrors the structure of `tokio::time` and can be used as a
//! drop-in replacement. Sleeps and intervals are registered as named entities
//! in the Moiré runtime graph so the dashboard can show which tasks are
//! suspended waiting for a timer to fire.
//!
//! # Available items
//!
//! | Item | Tokio equivalent |
//! |---|---|
//! | [`sleep`] | `tokio::time::sleep` |
//! | [`interval`] | `tokio::time::interval` |
//! | [`Interval`] | `tokio::time::Interval` |
use std::future::Future;
use std::time::Duration;

use moire_runtime::{instrument_future, EntityHandle};
use moire_types::FutureEntity;


/// Instrumented equivalent of [`tokio::time::sleep`].
pub fn sleep(duration: Duration) -> impl Future<Output = ()> {
    let handle = EntityHandle::new("time.sleep", FutureEntity {});

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
    handle: EntityHandle<FutureEntity>,
}

impl Interval {
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
    Interval {
        inner: tokio::time::interval(period),
        handle: EntityHandle::new("time.interval", FutureEntity {}),
    }
}
