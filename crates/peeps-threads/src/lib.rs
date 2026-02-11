//! Thread stack collection via `pthread_kill(SIGPROF)`.
//!
//! When the `diagnostics` feature is enabled, each OS thread registers itself
//! in a global registry. `collect_all_thread_stacks()` sends SIGPROF to every
//! registered thread and captures backtraces.
//!
//! On macOS, threads that don't respond to SIGPROF (e.g. blocked in kernel)
//! get a fallback backtrace via Mach thread APIs (suspend + frame pointer walk).
//!
//! When disabled, all functions are zero-cost no-ops.

pub use peeps_types::ThreadStackSnapshot;

/// RAII guard that deregisters the thread on drop.
pub struct ThreadGuard(());

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        deregister_thread();
    }
}

// ── Zero-cost stubs (no diagnostics) ─────────────────────────────

#[cfg(not(feature = "diagnostics"))]
mod imp {
    use super::*;

    #[inline(always)]
    pub fn install_sigprof_handler() {}

    #[inline(always)]
    pub fn register_thread(_name: &str) {}

    #[inline(always)]
    pub fn deregister_thread() {}

    #[inline(always)]
    pub fn register_thread_guard(_name: &str) -> ThreadGuard {
        ThreadGuard(())
    }

    #[inline(always)]
    pub fn collect_all_thread_stacks() -> Vec<ThreadStackSnapshot> {
        Vec::new()
    }
}

// ── Tracing implementation (diagnostics enabled) ─────────────────

#[cfg(feature = "diagnostics")]
mod imp {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    struct ThreadEntry {
        pthread_id: libc::pthread_t,
        name: String,
    }

    static THREAD_REGISTRY: OnceLock<Mutex<Vec<ThreadEntry>>> = OnceLock::new();

    fn registry() -> &'static Mutex<Vec<ThreadEntry>> {
        THREAD_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn pthread_eq(a: libc::pthread_t, b: libc::pthread_t) -> bool {
        unsafe { libc::pthread_equal(a, b) != 0 }
    }

    pub fn register_thread(name: &str) {
        let entry = ThreadEntry {
            pthread_id: unsafe { libc::pthread_self() },
            name: name.to_string(),
        };
        registry().lock().unwrap().push(entry);
    }

    pub fn deregister_thread() {
        let self_id = unsafe { libc::pthread_self() };
        if let Ok(mut reg) = registry().lock() {
            reg.retain(|e| !pthread_eq(e.pthread_id, self_id));
        }
    }

    pub fn register_thread_guard(name: &str) -> ThreadGuard {
        register_thread(name);
        ThreadGuard(())
    }

    // ── Stack collection machinery ───────────────────────────

    const MAX_THREADS: usize = 256;

    struct StackSlot {
        active: AtomicBool,
        pthread_id: Mutex<libc::pthread_t>,
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

    pub fn install_sigprof_handler() {
        let _ = slots();

        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = sigprof_handler as *const () as usize;
            sa.sa_flags = libc::SA_SIGINFO;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(libc::SIGPROF, &sa, std::ptr::null_mut());
        }
    }

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
                let bt = std::backtrace::Backtrace::force_capture();
                if let Ok(mut guard) = slot.backtrace.try_lock() {
                    *guard = Some(format!("{bt}"));
                }
                return;
            }
        }
    }

    const NUM_SAMPLES: u32 = 10;
    const SAMPLE_INTERVAL_MS: u64 = 10;
    const SAMPLE_TIMEOUT_MS: u64 = 500;

    fn sweep_once(thread_count: usize, self_id: libc::pthread_t) -> Vec<Option<String>> {
        let slots = slots();

        for slot in slots.iter().take(thread_count) {
            *slot.backtrace.lock().unwrap() = None;
        }

        let mut self_slot_idx = None;
        for (i, slot) in slots.iter().enumerate().take(thread_count) {
            if pthread_eq(*slot.pthread_id.lock().unwrap(), self_id) {
                self_slot_idx = Some(i);
                break;
            }
        }

        COLLECTION_IN_PROGRESS.store(true, Ordering::Release);

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

        let expected = if self_slot_idx.is_some() {
            thread_count - 1
        } else {
            thread_count
        };

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(SAMPLE_TIMEOUT_MS);
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

        let mut results: Vec<Option<String>> = (0..thread_count)
            .map(|i| {
                if self_slot_idx == Some(i) {
                    None
                } else {
                    slots[i].backtrace.lock().unwrap().take()
                }
            })
            .collect();

        // Mach fallback for unresponsive threads (macOS only)
        #[cfg(target_os = "macos")]
        {
            let reg = registry().lock().unwrap();
            for (i, result) in results.iter_mut().enumerate() {
                if result.is_some() {
                    continue; // already got a backtrace via SIGPROF
                }
                if self_slot_idx == Some(i) {
                    continue; // skip self
                }
                if let Some(entry) = reg.get(i) {
                    if let Some(bt) = mach_fallback::capture_thread_backtrace(entry.pthread_id) {
                        *result = Some(bt);
                    }
                }
            }
        }

        results
    }

    fn extract_top_frame(bt: &str) -> Option<String> {
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
            if trimmed.is_empty() || trimmed.starts_with("at ") || trimmed.starts_with("stack ") {
                continue;
            }
            let frame = if let Some(idx) = trimmed.find(": ") {
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
            if noise.iter().any(|n| frame.contains(n)) {
                continue;
            }

            return Some(frame.to_string());
        }
        None
    }

    pub fn collect_all_thread_stacks() -> Vec<ThreadStackSnapshot> {
        let reg = match registry().lock() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        if reg.is_empty() {
            return Vec::new();
        }

        let thread_count = reg.len().min(MAX_THREADS);

        let slots = slots();
        for (i, entry) in reg.iter().enumerate().take(thread_count) {
            *slots[i].pthread_id.lock().unwrap() = entry.pthread_id;
            slots[i].active.store(true, Ordering::Release);
        }

        let names: Vec<String> = reg
            .iter()
            .take(thread_count)
            .map(|e| e.name.clone())
            .collect();
        drop(reg);

        let self_id = unsafe { libc::pthread_self() };

        struct ThreadTracker {
            last_frame: Option<String>,
            same_count: u32,
            responded: u32,
            last_backtrace: Option<String>,
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

        for slot in slots.iter().take(thread_count) {
            slot.active.store(false, Ordering::Release);
        }

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
}

// ── Mach-based fallback for unresponsive threads (macOS) ─────────

#[cfg(all(feature = "diagnostics", target_os = "macos"))]
mod mach_fallback {
    const MAX_FRAMES: usize = 128;

    // Mach API types and functions not exposed by libc
    type KernReturn = libc::c_int;
    type MachPort = libc::c_uint;

    const KERN_SUCCESS: KernReturn = 0;

    #[cfg(target_arch = "aarch64")]
    const THREAD_STATE_FLAVOR: libc::c_int = 6; // ARM_THREAD_STATE64
    #[cfg(target_arch = "aarch64")]
    const THREAD_STATE_COUNT: u32 = 68; // sizeof(arm_thread_state64_t) / sizeof(u32)

    #[cfg(target_arch = "x86_64")]
    const THREAD_STATE_FLAVOR: libc::c_int = 4; // x86_THREAD_STATE64
    #[cfg(target_arch = "x86_64")]
    const THREAD_STATE_COUNT: u32 = 42; // sizeof(x86_thread_state64_t) / sizeof(u32)

    #[cfg(target_arch = "aarch64")]
    #[repr(C)]
    struct MachThreadState {
        x: [u64; 29],
        fp: u64,  // x29
        lr: u64,  // x30
        sp: u64,
        pc: u64,
        cpsr: u32,
        _pad: u32,
    }

    #[cfg(target_arch = "x86_64")]
    #[repr(C)]
    struct MachThreadState {
        rax: u64, rbx: u64, rcx: u64, rdx: u64,
        rdi: u64, rsi: u64, rbp: u64, rsp: u64,
        r8: u64, r9: u64, r10: u64, r11: u64,
        r12: u64, r13: u64, r14: u64, r15: u64,
        rip: u64, rflags: u64,
        cs: u64, fs: u64, gs: u64,
    }

    extern "C" {
        fn thread_suspend(target_thread: MachPort) -> KernReturn;
        fn thread_resume(target_thread: MachPort) -> KernReturn;
        fn thread_get_state(
            target_thread: MachPort,
            flavor: libc::c_int,
            old_state: *mut u8,
            old_state_count: *mut u32,
        ) -> KernReturn;
    }

    /// Capture a backtrace for a thread by suspending it and walking frame pointers.
    pub fn capture_thread_backtrace(pthread_id: libc::pthread_t) -> Option<String> {
        unsafe {
            let mach_thread = libc::pthread_mach_thread_np(pthread_id);
            if mach_thread == 0 {
                return None;
            }

            if thread_suspend(mach_thread) != KERN_SUCCESS {
                return None;
            }

            let frames = get_frames(mach_thread);

            thread_resume(mach_thread);

            if frames.is_empty() {
                return None;
            }

            Some(symbolize_frames(&frames))
        }
    }

    unsafe fn get_frames(mach_thread: MachPort) -> Vec<usize> {
        let (pc, fp) = get_pc_fp(mach_thread);
        if pc == 0 {
            return Vec::new();
        }

        let mut frames = Vec::with_capacity(MAX_FRAMES);
        frames.push(pc);

        // Walk frame pointer chain
        let mut current_fp = fp;
        while current_fp != 0 && frames.len() < MAX_FRAMES {
            // Frame layout: [saved_fp, return_addr]
            let saved_fp = *(current_fp as *const usize);
            let return_addr = *((current_fp as *const usize).add(1));

            if return_addr == 0 {
                break;
            }

            frames.push(return_addr);

            // Sanity: fp must grow (stack grows down, fp chain goes up)
            if saved_fp <= current_fp {
                break;
            }
            current_fp = saved_fp;
        }

        frames
    }

    unsafe fn get_pc_fp(mach_thread: MachPort) -> (usize, usize) {
        let mut state: MachThreadState = std::mem::zeroed();
        let mut count = THREAD_STATE_COUNT;
        let kr = thread_get_state(
            mach_thread,
            THREAD_STATE_FLAVOR,
            &mut state as *mut _ as *mut u8,
            &mut count,
        );
        if kr != KERN_SUCCESS {
            return (0, 0);
        }

        #[cfg(target_arch = "aarch64")]
        {
            (state.pc as usize, state.fp as usize)
        }
        #[cfg(target_arch = "x86_64")]
        {
            (state.rip as usize, state.rbp as usize)
        }
    }

    fn symbolize_frames(frames: &[usize]) -> String {
        let mut out = String::new();
        for (i, &addr) in frames.iter().enumerate() {
            let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
            let found = unsafe { libc::dladdr(addr as *const libc::c_void, &mut info) };
            if found != 0 && !info.dli_sname.is_null() {
                let name = unsafe { std::ffi::CStr::from_ptr(info.dli_sname) }
                    .to_string_lossy();
                let offset = addr - info.dli_saddr as usize;
                out.push_str(&format!("{i:>4}: {name}+0x{offset:x}\n"));
            } else {
                out.push_str(&format!("{i:>4}: 0x{addr:x}\n"));
            }
        }
        out
    }
}

// ── Public API (delegates to imp) ────────────────────────────────

/// Install the SIGPROF handler. No-op without `diagnostics`.
pub fn install_sigprof_handler() {
    imp::install_sigprof_handler();
}

/// Register the calling thread. No-op without `diagnostics`.
pub fn register_thread(name: &str) {
    imp::register_thread(name);
}

/// Deregister the calling thread. No-op without `diagnostics`.
pub fn deregister_thread() {
    imp::deregister_thread();
}

/// Register the calling thread and return a guard that deregisters on drop.
pub fn register_thread_guard(name: &str) -> ThreadGuard {
    imp::register_thread_guard(name)
}

/// Collect stack traces from all registered threads. Empty without `diagnostics`.
pub fn collect_all_thread_stacks() -> Vec<ThreadStackSnapshot> {
    imp::collect_all_thread_stacks()
}
