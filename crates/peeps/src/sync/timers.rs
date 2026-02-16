use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use facet::Facet;
use peeps_types::{Node, NodeKind};

// ── Attrs structs ─────────────────────────────────────

#[derive(Facet)]
struct SleepAttrs<'a> {
    name: &'a str,
    #[facet(rename = "wait.kind")]
    wait_kind: &'a str,
    deadline_ns: u64,
    elapsed_ns: u64,
    meta: TimerMeta<'a>,
}

#[derive(Facet)]
struct IntervalAttrs<'a> {
    name: &'a str,
    #[facet(rename = "wait.kind")]
    wait_kind: &'a str,
    period_ms: u64,
    tick_count: u64,
    elapsed_ns: u64,
    meta: TimerMeta<'a>,
}

#[derive(Facet)]
struct TimeoutAttrs<'a> {
    name: &'a str,
    #[facet(rename = "wait.kind")]
    wait_kind: &'a str,
    deadline_ns: u64,
    elapsed_ns: u64,
    meta: TimerMeta<'a>,
}

#[derive(Facet)]
struct TimerMeta<'a> {
    #[facet(rename = "ctx.location")]
    ctx_location: &'a str,
}

// ── Sleep ───────────────────────────────────────────────

struct SleepInfo {
    name: String,
    node_id: String,
    location: String,
    deadline_ns: u64,
    created_at: Instant,
}

static SLEEP_REGISTRY: LazyLock<Mutex<Vec<Weak<SleepInfo>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn prune_and_register_sleep(info: &Arc<SleepInfo>) {
    let mut reg = SLEEP_REGISTRY.lock().unwrap();
    reg.retain(|w| w.strong_count() > 0);
    reg.push(Arc::downgrade(info));
}

/// A diagnostic wrapper around `tokio::time::Sleep`.
pub struct DiagnosticSleep {
    inner: Pin<Box<tokio::time::Sleep>>,
    info: Arc<SleepInfo>,
    edge_src: Option<String>,
    pending: bool,
}

impl DiagnosticSleep {
    fn new_inner(
        duration: Duration,
        inner: Pin<Box<tokio::time::Sleep>>,
        name: String,
        location: String,
    ) -> Self {
        let deadline_ns = duration.as_nanos().min(u64::MAX as u128) as u64;
        let info = Arc::new(SleepInfo {
            name,
            node_id: peeps_types::new_node_id("sleep"),
            location,
            deadline_ns,
            created_at: Instant::now(),
        });
        prune_and_register_sleep(&info);
        Self {
            inner,
            info,
            edge_src: None,
            pending: false,
        }
    }
}

impl Future for DiagnosticSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = unsafe { self.get_unchecked_mut() };
        match this.inner.as_mut().poll(cx) {
            Poll::Ready(()) => {
                if let Some(ref src) = this.edge_src.take() {
                    crate::registry::remove_edge(src, &this.info.node_id);
                }
                Poll::Ready(())
            }
            Poll::Pending => {
                if !this.pending {
                    this.pending = true;
                    crate::stack::with_top(|top| {
                        crate::registry::edge(top, &this.info.node_id);
                        this.edge_src = Some(top.to_string());
                    });
                }
                Poll::Pending
            }
        }
    }
}

impl Drop for DiagnosticSleep {
    fn drop(&mut self) {
        if let Some(ref src) = self.edge_src {
            crate::registry::remove_edge(src, &self.info.node_id);
        }
    }
}

/// Diagnostic wrapper for `tokio::time::sleep`.
#[track_caller]
pub fn sleep(duration: Duration) -> DiagnosticSleep {
    let caller = std::panic::Location::caller();
    let location = crate::caller_location(caller);
    let label = format!("sleep({}ms)", duration.as_millis());
    DiagnosticSleep::new_inner(
        duration,
        Box::pin(tokio::time::sleep(duration)),
        label,
        location,
    )
}

// ── Interval ────────────────────────────────────────────

struct IntervalInfo {
    name: String,
    node_id: String,
    location: String,
    period_ms: u64,
    tick_count: AtomicU64,
    created_at: Instant,
}

static INTERVAL_REGISTRY: LazyLock<Mutex<Vec<Weak<IntervalInfo>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn prune_and_register_interval(info: &Arc<IntervalInfo>) {
    let mut reg = INTERVAL_REGISTRY.lock().unwrap();
    reg.retain(|w| w.strong_count() > 0);
    reg.push(Arc::downgrade(info));
}

/// A diagnostic wrapper around `tokio::time::Interval`.
pub struct DiagnosticInterval {
    inner: tokio::time::Interval,
    info: Arc<IntervalInfo>,
}

impl DiagnosticInterval {
    /// Completes when the next instant in the interval has been reached.
    pub async fn tick(&mut self) -> tokio::time::Instant {
        let mut edge_src: Option<String> = None;
        crate::stack::with_top(|src| {
            edge_src = Some(src.to_string());
            crate::registry::edge(src, &self.info.node_id);
        });

        let instant = self.inner.tick().await;

        if let Some(ref src) = edge_src {
            crate::registry::remove_edge(src, &self.info.node_id);
        }
        self.info.tick_count.fetch_add(1, Ordering::Relaxed);
        instant
    }

    /// Resets the interval to complete one period after the current time.
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Returns the period of the interval.
    pub fn period(&self) -> Duration {
        self.inner.period()
    }
}

/// Diagnostic wrapper for `tokio::time::interval`.
#[track_caller]
pub fn interval(period: Duration) -> DiagnosticInterval {
    let caller = std::panic::Location::caller();
    let location = crate::caller_location(caller);
    let label = format!("interval({}ms)", period.as_millis());
    let info = Arc::new(IntervalInfo {
        name: label,
        node_id: peeps_types::new_node_id("interval"),
        location,
        period_ms: period.as_millis().min(u64::MAX as u128) as u64,
        tick_count: AtomicU64::new(0),
        created_at: Instant::now(),
    });
    prune_and_register_interval(&info);
    DiagnosticInterval {
        inner: tokio::time::interval(period),
        info,
    }
}

/// Diagnostic wrapper for `tokio::time::interval_at`.
#[track_caller]
pub fn interval_at(start: tokio::time::Instant, period: Duration) -> DiagnosticInterval {
    let caller = std::panic::Location::caller();
    let location = crate::caller_location(caller);
    let label = format!("interval({}ms)", period.as_millis());
    let info = Arc::new(IntervalInfo {
        name: label,
        node_id: peeps_types::new_node_id("interval"),
        location,
        period_ms: period.as_millis().min(u64::MAX as u128) as u64,
        tick_count: AtomicU64::new(0),
        created_at: Instant::now(),
    });
    prune_and_register_interval(&info);
    DiagnosticInterval {
        inner: tokio::time::interval_at(start, period),
        info,
    }
}

// ── Timeout ─────────────────────────────────────────────

struct TimeoutInfo {
    name: String,
    node_id: String,
    location: String,
    deadline_ns: u64,
    created_at: Instant,
}

static TIMEOUT_REGISTRY: LazyLock<Mutex<Vec<Weak<TimeoutInfo>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn prune_and_register_timeout(info: &Arc<TimeoutInfo>) {
    let mut reg = TIMEOUT_REGISTRY.lock().unwrap();
    reg.retain(|w| w.strong_count() > 0);
    reg.push(Arc::downgrade(info));
}

/// A diagnostic wrapper around `tokio::time::Timeout`.
pub struct DiagnosticTimeout<F> {
    inner: Pin<Box<tokio::time::Timeout<F>>>,
    info: Arc<TimeoutInfo>,
    edge_src: Option<String>,
    pending: bool,
}

impl<F: Future> Future for DiagnosticTimeout<F> {
    type Output = Result<F::Output, tokio::time::error::Elapsed>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        match this.inner.as_mut().poll(cx) {
            Poll::Ready(result) => {
                if let Some(ref src) = this.edge_src.take() {
                    crate::registry::remove_edge(src, &this.info.node_id);
                }
                Poll::Ready(result)
            }
            Poll::Pending => {
                if !this.pending {
                    this.pending = true;
                    crate::stack::with_top(|top| {
                        crate::registry::edge(top, &this.info.node_id);
                        this.edge_src = Some(top.to_string());
                    });
                }
                Poll::Pending
            }
        }
    }
}

impl<F> Drop for DiagnosticTimeout<F> {
    fn drop(&mut self) {
        if let Some(ref src) = self.edge_src {
            crate::registry::remove_edge(src, &self.info.node_id);
        }
    }
}

/// Diagnostic wrapper for `tokio::time::timeout`.
#[track_caller]
pub fn timeout<F: Future>(duration: Duration, future: F) -> DiagnosticTimeout<F> {
    let caller = std::panic::Location::caller();
    let location = crate::caller_location(caller);
    let deadline_ns = duration.as_nanos().min(u64::MAX as u128) as u64;
    let label = format!("timeout({}ms)", duration.as_millis());
    let info = Arc::new(TimeoutInfo {
        name: label,
        node_id: peeps_types::new_node_id("timeout"),
        location,
        deadline_ns,
        created_at: Instant::now(),
    });
    prune_and_register_timeout(&info);
    DiagnosticTimeout {
        inner: Box::pin(tokio::time::timeout(duration, future)),
        info,
        edge_src: None,
        pending: false,
    }
}

// ── Graph emission ──────────────────────────────────────

pub(super) fn emit_sleep_nodes(graph: &mut peeps_types::GraphSnapshot) {
    let now = Instant::now();
    let reg = SLEEP_REGISTRY.lock().unwrap();

    for info in reg.iter().filter_map(|w| w.upgrade()) {
        let elapsed_ns =
            (now.duration_since(info.created_at).as_nanos().min(u64::MAX as u128)) as u64;

        let attrs = SleepAttrs {
            name: &info.name,
            wait_kind: "sleep",
            deadline_ns: info.deadline_ns,
            elapsed_ns,
            meta: TimerMeta {
                ctx_location: &info.location,
            },
        };

        graph.nodes.push(Node {
            id: info.node_id.clone(),
            kind: NodeKind::Sleep,
            label: Some(info.name.clone()),
            attrs_json: facet_json::to_string(&attrs).unwrap(),
        });
    }
}

pub(super) fn emit_interval_nodes(graph: &mut peeps_types::GraphSnapshot) {
    let now = Instant::now();
    let reg = INTERVAL_REGISTRY.lock().unwrap();

    for info in reg.iter().filter_map(|w| w.upgrade()) {
        let elapsed_ns =
            (now.duration_since(info.created_at).as_nanos().min(u64::MAX as u128)) as u64;
        let tick_count = info.tick_count.load(Ordering::Relaxed);

        let attrs = IntervalAttrs {
            name: &info.name,
            wait_kind: "interval",
            period_ms: info.period_ms,
            tick_count,
            elapsed_ns,
            meta: TimerMeta {
                ctx_location: &info.location,
            },
        };

        graph.nodes.push(Node {
            id: info.node_id.clone(),
            kind: NodeKind::Interval,
            label: Some(info.name.clone()),
            attrs_json: facet_json::to_string(&attrs).unwrap(),
        });
    }
}

pub(super) fn emit_timeout_nodes(graph: &mut peeps_types::GraphSnapshot) {
    let now = Instant::now();
    let reg = TIMEOUT_REGISTRY.lock().unwrap();

    for info in reg.iter().filter_map(|w| w.upgrade()) {
        let elapsed_ns =
            (now.duration_since(info.created_at).as_nanos().min(u64::MAX as u128)) as u64;

        let attrs = TimeoutAttrs {
            name: &info.name,
            wait_kind: "timeout",
            deadline_ns: info.deadline_ns,
            elapsed_ns,
            meta: TimerMeta {
                ctx_location: &info.location,
            },
        };

        graph.nodes.push(Node {
            id: info.node_id.clone(),
            kind: NodeKind::Timeout,
            label: Some(info.name.clone()),
            attrs_json: facet_json::to_string(&attrs).unwrap(),
        });
    }
}
