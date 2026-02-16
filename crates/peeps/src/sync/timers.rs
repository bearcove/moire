use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};

use facet::Facet;
use peeps_types::{Node, NodeKind};

// ── Attrs structs ─────────────────────────────────────

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
struct TimerMeta<'a> {
    #[facet(rename = "ctx.location")]
    ctx_location: &'a str,
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

    /// Sets the missed tick behavior.
    pub fn set_missed_tick_behavior(&mut self, behavior: tokio::time::MissedTickBehavior) {
        self.inner.set_missed_tick_behavior(behavior);
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

// ── Graph emission ──────────────────────────────────────

pub(super) fn emit_interval_nodes(graph: &mut peeps_types::GraphSnapshot) {
    let now = Instant::now();
    let reg = INTERVAL_REGISTRY.lock().unwrap();

    for info in reg.iter().filter_map(|w| w.upgrade()) {
        let elapsed_ns = (now
            .duration_since(info.created_at)
            .as_nanos()
            .min(u64::MAX as u128)) as u64;
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
