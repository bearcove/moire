//! Thread stack collection via `pthread_kill(SIGPROF)`.
//!
//! Each OS thread registers itself in a global registry. When diagnostics
//! are requested (via SIGUSR1 handler, which runs on a tokio thread),
//! `collect_all_thread_stacks()` sends SIGPROF to every registered thread.
//! The SIGPROF handler captures a backtrace and stashes it in a pre-allocated
//! slot. After a short wait, the collector reads all slots.
//!
//! This is the same technique used by pprof-rs and Servo's hang monitor.
//! `Backtrace::force_capture()` in a signal handler is not strictly
//! signal-safe, but works in practice on Linux and macOS.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use facet::Facet;

// ── Snapshot type ────────────────────────────────────────────

/// A single thread's stack trace, with sampling data for stuck detection.
#[derive(Debug, Clone, Facet)]
pub struct ThreadStackSnapshot {
    /// Thread name (e.g. "tokio-runtime-worker", "main", "vfs-worker").
    pub name: String,
    /// Most recent captured backtrace. `None` if the thread never responded
    /// (possibly stuck in an uninterruptible syscall).
    pub backtrace: Option<String>,
    /// Number of samples taken (e.g. 10).
    pub samples: u32,
    /// Number of samples where the thread responded with a backtrace.
    pub responded: u32,
    /// Number of samples where the top useful frame was identical to the
    /// previous sample. High values (close to `samples`) indicate the thread
    /// is stuck or parked in the same location.
    pub same_location_count: u32,
    /// The "top useful frame" that appeared most often across samples.
    /// Used by the dashboard to classify threads as stuck/idle/active.
    pub dominant_frame: Option<String>,
}

// ── Thread registry ──────────────────────────────────────────

struct ThreadEntry {
    pthread_id: libc::pthread_t,
    name: String,
}

static THREAD_REGISTRY: OnceLock<Mutex<Vec<ThreadEntry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<ThreadEntry>> {
    THREAD_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// Register the calling thread in the global thread registry.
/// Call this from thread startup (e.g. `on_thread_start` callback).
pub fn register_thread(name: &str) {
    let entry = ThreadEntry {
        pthread_id: unsafe { libc::pthread_self() },
        name: name.to_string(),
    };
    registry().lock().unwrap().push(entry);
}

/// Deregister the calling thread from the global thread registry.
/// Call this from thread teardown (e.g. `on_thread_stop` callback).
pub fn deregister_thread() {
    let self_id = unsafe { libc::pthread_self() };
    if let Ok(mut reg) = registry().lock() {
        reg.retain(|e| !pthread_eq(e.pthread_id, self_id));
    }
}

/// RAII guard that deregisters the thread on drop.
pub struct ThreadGuard(());

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        deregister_thread();
    }
}

/// Register the calling thread and return a guard that deregisters on drop.
pub fn register_thread_guard(name: &str) -> ThreadGuard {
    register_thread(name);
    ThreadGuard(())
}

fn pthread_eq(a: libc::pthread_t, b: libc::pthread_t) -> bool {
    unsafe { libc::pthread_equal(a, b) != 0 }
}

// ── Stack collection machinery ───────────────────────────────

/// Maximum number of threads we can collect stacks from simultaneously.
const MAX_THREADS: usize = 256;

struct StackSlot {
    /// Whether this slot is active (waiting for a response).
    active: AtomicBool,
    /// The pthread_id this slot is assigned to (set before signaling).
    pthread_id: Mutex<libc::pthread_t>,
    /// The captured backtrace (written by the SIGPROF handler).
    backtrace: Mutex<Option<String>>,
}

static STACK_SLOTS: OnceLock<Vec<StackSlot>> = OnceLock::new();
static COLLECTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

fn slots() -> &'static Vec<StackSlot> {
    STACK_SLOTS.get_or_init(|| {
        (0..MAX_THREADS)
            .map(|_| StackSlot {
                active: AtomicBool::new(false),
                pthread_id: Mutex::new(0),
                backtrace: Mutex::new(None),
            })
            .collect()
    })
}

/// Install the SIGPROF handler. Call once at process startup,
/// before any threads are spawned.
pub fn install_sigprof_handler() {
    // Initialize slots eagerly so the signal handler can access them.
    let _ = slots();

    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = sigprof_handler as *const () as usize;
        sa.sa_flags = libc::SA_SIGINFO;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGPROF, &sa, std::ptr::null_mut());
    }
}

/// The SIGPROF signal handler.
///
/// Runs in signal context on the target thread. Captures a backtrace
/// and stores it in the pre-assigned slot.
extern "C" fn sigprof_handler(
    _sig: libc::c_int,
    _info: *mut libc::siginfo_t,
    _ctx: *mut libc::c_void,
) {
    if !COLLECTION_IN_PROGRESS.load(Ordering::Acquire) {
        return;
    }

    let self_id = unsafe { libc::pthread_self() };
    let slots = match STACK_SLOTS.get() {
        Some(s) => s,
        None => return,
    };

    for slot in slots.iter() {
        if !slot.active.load(Ordering::Acquire) {
            continue;
        }
        let matches = match slot.pthread_id.try_lock() {
            Ok(id) => pthread_eq(*id, self_id),
            Err(_) => false,
        };
        if matches {
            // Capture backtrace. Not strictly signal-safe but works in practice
            // (same technique as pprof-rs).
            let bt = std::backtrace::Backtrace::force_capture();
            if let Ok(mut guard) = slot.backtrace.try_lock() {
                *guard = Some(format!("{bt}"));
            }
            return;
        }
    }
}

/// Number of samples to take when collecting thread stacks.
const NUM_SAMPLES: u32 = 10;
/// Interval between samples in milliseconds.
const SAMPLE_INTERVAL_MS: u64 = 10;
/// Timeout waiting for a single sample sweep to complete (ms).
const SAMPLE_TIMEOUT_MS: u64 = 100;

/// Perform a single SIGPROF sweep: signal all threads and wait for responses.
/// Returns one `Option<String>` per thread slot (index-aligned with the registry).
/// `self_id` is the pthread_t of the calling thread — we skip signaling it to
/// avoid capturing our own collection machinery as a "stuck" thread.
fn sweep_once(thread_count: usize, self_id: libc::pthread_t) -> Vec<Option<String>> {
    let slots = slots();

    // Clear backtrace slots
    for slot in slots.iter().take(thread_count) {
        *slot.backtrace.lock().unwrap() = None;
    }

    // Mark our own slot as already responded (with None) so we don't wait for it
    let mut self_slot_idx = None;
    for (i, slot) in slots.iter().enumerate().take(thread_count) {
        if pthread_eq(*slot.pthread_id.lock().unwrap(), self_id) {
            self_slot_idx = Some(i);
            break;
        }
    }

    // Begin collection — the SIGPROF handler checks this flag
    COLLECTION_IN_PROGRESS.store(true, Ordering::Release);

    // Send SIGPROF to each registered thread, skipping ourselves
    let reg = registry().lock().unwrap();
    for entry in reg.iter().take(thread_count) {
        if pthread_eq(entry.pthread_id, self_id) {
            continue;
        }
        unsafe {
            libc::pthread_kill(entry.pthread_id, libc::SIGPROF);
        }
    }
    drop(reg);

    // Expected responses: all except ourselves
    let expected = if self_slot_idx.is_some() {
        thread_count - 1
    } else {
        thread_count
    };

    // Wait for responses
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(SAMPLE_TIMEOUT_MS);
    loop {
        let responded = (0..thread_count)
            .filter(|&i| {
                slots[i]
                    .backtrace
                    .try_lock()
                    .map(|g| g.is_some())
                    .unwrap_or(false)
            })
            .count();
        if responded >= expected || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    COLLECTION_IN_PROGRESS.store(false, Ordering::Release);

    // Read results — None for our own slot (we didn't signal ourselves)
    (0..thread_count)
        .map(|i| {
            if self_slot_idx == Some(i) {
                None
            } else {
                slots[i].backtrace.lock().unwrap().take()
            }
        })
        .collect()
}

/// Extract the first "useful" frame from a backtrace string.
///
/// Skips noise frames (signal handler internals, std::backtrace machinery,
/// tokio runtime internals for parking) and returns the first frame that
/// looks like application or library code.
fn extract_top_frame(bt: &str) -> Option<String> {
    // Backtrace lines look like:
    //   4: module::function::h1234abcd
    // or:
    //   at /path/to/file.rs:123
    // We want the "module::function" lines.
    let noise = [
        "std::backtrace",
        "std::sys",
        "backtrace_rs",
        "backtrace::",
        "sigprof_handler",
        "__os_lock",
        "_pthread_",
        "pthread_",
        "core::ops::function",
        "std::panicking",
        "std::panic",
        "std::rt::lang_start",
        "<unknown>",
        "mio::poll",
        "mio::sys",
        "__psynch_",
        "kevent",
        "_sigtramp",
        "signal_handler",
        "vx_dump::thread_stacks",
    ];

    for line in bt.lines() {
        let trimmed = line.trim();
        // Skip lines that are just numbers, "at" lines, or empty
        if trimmed.is_empty() || trimmed.starts_with("at ") || trimmed.starts_with("stack ") {
            continue;
        }
        // Strip leading frame number like "4: "
        let frame = if let Some(idx) = trimmed.find(": ") {
            // Check if prefix is a number
            if trimmed[..idx].trim().chars().all(|c| c.is_ascii_digit()) {
                trimmed[idx + 2..].trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        if frame.is_empty() {
            continue;
        }
        // Skip noise
        if noise.iter().any(|n| frame.contains(n)) {
            continue;
        }

        return Some(frame.to_string());
    }
    None
}

/// Collect stack traces from all registered threads by sampling multiple times.
///
/// Takes `NUM_SAMPLES` snapshots at `SAMPLE_INTERVAL_MS` intervals.
/// For each thread, tracks how often the top frame stays the same across
/// samples. Threads stuck in the same frame across all samples are "stuck";
/// threads that move around are "active."
///
/// This is called from the SIGUSR1 handler, which runs on a tokio worker
/// thread (NOT in signal context), so it can safely block and sleep.
pub fn collect_all_thread_stacks() -> Vec<ThreadStackSnapshot> {
    let reg = match registry().lock() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    if reg.is_empty() {
        return Vec::new();
    }

    let thread_count = reg.len().min(MAX_THREADS);

    // Prepare slots with pthread_ids
    let slots = slots();
    for (i, entry) in reg.iter().enumerate().take(thread_count) {
        *slots[i].pthread_id.lock().unwrap() = entry.pthread_id;
        slots[i].active.store(true, Ordering::Release);
    }

    // Collect thread names before dropping the lock
    let names: Vec<String> = reg
        .iter()
        .take(thread_count)
        .map(|e| e.name.clone())
        .collect();
    drop(reg);

    // Remember our own pthread_t so we skip signaling ourselves
    let self_id = unsafe { libc::pthread_self() };

    // Per-thread tracking across samples
    struct ThreadTracker {
        last_frame: Option<String>,
        same_count: u32,
        responded: u32,
        last_backtrace: Option<String>,
        // Count of each unique top frame
        frame_counts: std::collections::HashMap<String, u32>,
    }
    let mut trackers: Vec<ThreadTracker> = (0..thread_count)
        .map(|_| ThreadTracker {
            last_frame: None,
            same_count: 0,
            responded: 0,
            last_backtrace: None,
            frame_counts: std::collections::HashMap::new(),
        })
        .collect();

    // Take NUM_SAMPLES sweeps
    for _ in 0..NUM_SAMPLES {
        let results = sweep_once(thread_count, self_id);

        for (i, bt) in results.into_iter().enumerate() {
            let tracker = &mut trackers[i];
            if let Some(ref bt_str) = bt {
                tracker.responded += 1;
                tracker.last_backtrace = Some(bt_str.clone());
                let frame = extract_top_frame(bt_str);
                if let Some(ref f) = frame {
                    *tracker.frame_counts.entry(f.clone()).or_insert(0) += 1;
                }
                if frame == tracker.last_frame && frame.is_some() {
                    tracker.same_count += 1;
                }
                tracker.last_frame = frame;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(SAMPLE_INTERVAL_MS));
    }

    // Deactivate all slots
    for slot in slots.iter().take(thread_count) {
        slot.active.store(false, Ordering::Release);
    }

    // Build results
    names
        .into_iter()
        .zip(trackers)
        .map(|(name, tracker)| {
            let dominant_frame = tracker
                .frame_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(frame, _)| frame);

            ThreadStackSnapshot {
                name,
                backtrace: tracker.last_backtrace,
                samples: NUM_SAMPLES,
                responded: tracker.responded,
                same_location_count: tracker.same_count,
                dominant_frame,
            }
        })
        .collect()
}
