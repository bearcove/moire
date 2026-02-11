//! Task instrumentation for Tokio spawned tasks.
//!
//! Wraps spawned tasks to capture timing, poll events, and backtraces.
//! Dumps are included in the process dump JSON for debugging stuck tasks.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use backtrace::Backtrace;
use facet::Facet;

/// Global task ID counter.
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Global registry of active tasks.
static TASK_REGISTRY: Mutex<Option<Vec<Arc<TaskInfo>>>> = Mutex::new(None);

/// Unique task ID.
pub type TaskId = u64;

/// Snapshot of a tracked task for diagnostics.
#[derive(Debug, Clone, Facet)]
pub struct TaskSnapshot {
    /// Unique task ID.
    pub id: TaskId,
    /// Task name provided at spawn.
    pub name: String,
    /// Task state.
    pub state: TaskState,
    /// Time when task was spawned.
    pub spawned_at_secs: f64,
    /// Time elapsed since spawn (seconds).
    pub age_secs: f64,
    /// Backtrace captured at spawn location.
    pub spawn_backtrace: String,
    /// Poll events (most recent first, limited to last 16).
    pub poll_events: Vec<PollEvent>,
}

/// Task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum TaskState {
    /// Task is pending (not currently polling).
    Pending,
    /// Task is currently being polled.
    Polling,
    /// Task has completed.
    Completed,
}

/// A single poll event.
#[derive(Debug, Clone, Facet)]
pub struct PollEvent {
    /// Time when poll started (seconds since spawn).
    pub started_at_secs: f64,
    /// Poll duration (seconds), None if still polling.
    pub duration_secs: Option<f64>,
    /// Result of the poll.
    pub result: PollResult,
    /// Backtrace captured at poll site (if diagnostics enabled).
    pub backtrace: Option<String>,
}

/// Poll result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum PollResult {
    /// Poll returned Pending.
    Pending,
    /// Poll returned Ready.
    Ready,
}

/// Internal task state shared between wrapper and registry.
struct TaskInfo {
    id: TaskId,
    name: String,
    spawned_at: Instant,
    spawn_backtrace: String,
    state: Mutex<TaskInfoState>,
}

struct TaskInfoState {
    state: TaskState,
    /// Limited to last 16 poll events to avoid unbounded memory.
    poll_events: Vec<PollEventInternal>,
}

struct PollEventInternal {
    started_at: Instant,
    duration: Option<std::time::Duration>,
    result: PollResult,
    backtrace: Option<String>,
}

impl TaskInfo {
    fn snapshot(&self, now: Instant) -> TaskSnapshot {
        let state = self.state.lock().unwrap();
        let age = now.duration_since(self.spawned_at);

        TaskSnapshot {
            id: self.id,
            name: self.name.clone(),
            state: state.state,
            spawned_at_secs: self.spawned_at.elapsed().as_secs_f64() - age.as_secs_f64(),
            age_secs: age.as_secs_f64(),
            spawn_backtrace: self.spawn_backtrace.clone(),
            poll_events: state
                .poll_events
                .iter()
                .map(|e| PollEvent {
                    started_at_secs: e.started_at.duration_since(self.spawned_at).as_secs_f64(),
                    duration_secs: e.duration.map(|d| d.as_secs_f64()),
                    result: e.result,
                    backtrace: e.backtrace.clone(),
                })
                .collect(),
        }
    }

    fn record_poll_start(&self, backtrace: Option<String>) {
        let mut state = self.state.lock().unwrap();
        state.state = TaskState::Polling;

        // Keep only last 15 events to make room for new one
        if state.poll_events.len() >= 16 {
            state.poll_events.remove(0);
        }

        state.poll_events.push(PollEventInternal {
            started_at: Instant::now(),
            duration: None,
            result: PollResult::Pending, // Will be updated on poll end
            backtrace,
        });
    }

    fn record_poll_end(&self, result: PollResult) {
        let mut state = self.state.lock().unwrap();

        if let Some(last) = state.poll_events.last_mut() {
            last.duration = Some(last.started_at.elapsed());
            last.result = result;
        }

        state.state = match result {
            PollResult::Ready => TaskState::Completed,
            PollResult::Pending => TaskState::Pending,
        };
    }
}

/// Future wrapper that records poll events.
struct TrackedFuture<F> {
    inner: F,
    task_info: Arc<TaskInfo>,
    capture_backtraces: bool,
}

impl<F: Future> Future for TrackedFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: We're projecting through TrackedFuture to access the inner future.
        // TrackedFuture is not Unpin, but we never move out of inner.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { Pin::new_unchecked(&mut this.inner) };

        let backtrace = if this.capture_backtraces {
            Some(format!("{:?}", Backtrace::new()))
        } else {
            None
        };

        this.task_info.record_poll_start(backtrace);

        let result = inner.poll(cx);

        let poll_result = match result {
            Poll::Ready(_) => PollResult::Ready,
            Poll::Pending => PollResult::Pending,
        };

        this.task_info.record_poll_end(poll_result);

        result
    }
}

/// Initialize the task tracking registry.
///
/// Must be called once at process startup before spawning any tracked tasks.
pub fn init_task_tracking() {
    *TASK_REGISTRY.lock().unwrap() = Some(Vec::new());
}

/// Spawn a tracked task with the given name.
///
/// Captures spawn backtrace and records poll events.
/// Returns a JoinHandle to the spawned task.
pub fn spawn_tracked<F>(name: impl Into<String>, future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let name = name.into();
    let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);

    let spawn_backtrace = format!("{:?}", Backtrace::new());

    let task_info = Arc::new(TaskInfo {
        id: task_id,
        name,
        spawned_at: Instant::now(),
        spawn_backtrace,
        state: Mutex::new(TaskInfoState {
            state: TaskState::Pending,
            poll_events: Vec::new(),
        }),
    });

    // Register in global registry
    if let Some(registry) = TASK_REGISTRY.lock().unwrap().as_mut() {
        registry.push(Arc::clone(&task_info));
    }

    // Always capture backtraces on poll to aid debugging stuck tasks
    let capture_backtraces = true;

    let tracked = TrackedFuture {
        inner: future,
        task_info,
        capture_backtraces,
    };

    tokio::spawn(tracked)
}

/// Collect snapshots of all tracked tasks.
///
/// Called during process dump collection.
pub fn snapshot_all_tasks() -> Vec<TaskSnapshot> {
    let now = Instant::now();

    let registry = TASK_REGISTRY.lock().unwrap();
    let Some(tasks) = registry.as_ref() else {
        return Vec::new();
    };

    // Remove completed tasks older than 30 seconds from the registry
    let cutoff = now - std::time::Duration::from_secs(30);

    tasks
        .iter()
        .filter(|task| {
            let state = task.state.lock().unwrap();
            state.state != TaskState::Completed || task.spawned_at > cutoff
        })
        .map(|task| task.snapshot(now))
        .collect()
}

/// Remove completed tasks from the registry (called periodically to avoid unbounded growth).
pub fn cleanup_completed_tasks() {
    let now = Instant::now();
    let cutoff = now - std::time::Duration::from_secs(30);

    if let Some(registry) = TASK_REGISTRY.lock().unwrap().as_mut() {
        registry.retain(|task| {
            let state = task.state.lock().unwrap();
            state.state != TaskState::Completed || task.spawned_at > cutoff
        });
    }
}
