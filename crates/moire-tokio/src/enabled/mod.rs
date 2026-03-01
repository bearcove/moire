pub mod custom;
pub mod fs;
pub mod process;
pub mod rpc;
pub mod sync;
pub mod task;
pub mod time;

pub use task::{spawn, spawn_blocking};

#[doc(hidden)]
pub mod __internal {
    pub use moire_runtime::{InstrumentedFuture, instrument_future};
}
