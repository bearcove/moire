pub(crate) mod channels;
pub(crate) mod joinset;
pub(crate) mod process;
pub(crate) mod rpc;
pub(crate) mod sync;

pub use self::channels::*;
pub use self::process::*;
pub use self::rpc::*;
pub use self::sync::*;
pub use moire_runtime::*;

use core::sync::atomic::{AtomicU64, Ordering};
use moire_trace_capture::{capture_current, trace_capabilities, CaptureOptions};
use moire_trace_types::BacktraceId;

static NEXT_BACKTRACE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn capture_backtrace_id() -> SourceId {
    let capabilities = trace_capabilities();
    assert!(
        capabilities.trace_v1,
        "trace capture prerequisites missing: trace_v1 unsupported on this platform"
    );

    let raw = NEXT_BACKTRACE_ID.fetch_add(1, Ordering::Relaxed);
    let backtrace_id = BacktraceId::new(raw)
        .expect("backtrace id invariant violated: generated id must be non-zero");

    capture_current(backtrace_id, CaptureOptions::default()).unwrap_or_else(|err| {
        panic!(
            "failed to capture backtrace for enabled API boundary: {err}"
        )
    });

    SourceId::new(backtrace_id.get()).unwrap_or_else(|_| {
        panic!(
            "failed to derive runtime source id from captured backtrace id {}; SourceId requires JSON-safe U53 range",
            backtrace_id.get()
        )
    })
}
