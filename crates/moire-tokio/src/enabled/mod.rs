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
use std::sync::Once;

static NEXT_BACKTRACE_ID: AtomicU64 = AtomicU64::new(1);
static DIAGNOSTICS_INIT_ONCE: Once = Once::new();

// r[impl process.auto-init]
#[used]
#[cfg_attr(target_os = "macos", link_section = "__DATA,__mod_init_func")]
#[cfg_attr(
    any(target_os = "linux", target_os = "android", target_os = "freebsd"),
    link_section = ".init_array"
)]
static INIT_DIAGNOSTICS_RUNTIME: extern "C" fn() = {
    extern "C" fn init() {
        init_diagnostics_runtime_once();
    }
    init
};

fn init_diagnostics_runtime_once() {
    DIAGNOSTICS_INIT_ONCE.call_once(|| {
        moire_trace_capture::validate_frame_pointers_or_panic();
        init_runtime_from_macro();
    });
}

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
        panic!("failed to capture backtrace for enabled API boundary: {err}")
    });

    SourceId::new(backtrace_id.get()).unwrap_or_else(|_| {
        panic!(
            "failed to derive runtime source id from captured backtrace id {}; SourceId requires JSON-safe U53 range",
            backtrace_id.get()
        )
    })
}
