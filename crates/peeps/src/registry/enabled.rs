use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, OnceLock};

use peeps_types::{Edge, GraphSnapshot};

// ── Process metadata ─────────────────────────────────────

struct ProcessInfo {
    name: String,
    proc_key: String,
}

static PROCESS_INFO: OnceLock<ProcessInfo> = OnceLock::new();

// ── Canonical edge storage ───────────────────────────────
//
// Stores `needs` edges emitted via `stack::with_top(|src| registry::edge(src, dst))`.
// These represent the current wait graph: which futures are waiting on which resources.

static EDGES: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

// ── Initialization ───────────────────────────────────────

/// Initialize process metadata for the registry.
///
/// Should be called once at startup. Subsequent calls are ignored (first write wins).
pub(crate) fn init(process_name: &str, proc_key: &str) {
    let _ = PROCESS_INFO.set(ProcessInfo {
        name: process_name.to_string(),
        proc_key: proc_key.to_string(),
    });
}

// ── Accessors ────────────────────────────────────────────

pub(crate) fn process_name() -> Option<&'static str> {
    PROCESS_INFO.get().map(|p| p.name.as_str())
}

pub(crate) fn proc_key() -> Option<&'static str> {
    PROCESS_INFO.get().map(|p| p.proc_key.as_str())
}

// ── Edge tracking ────────────────────────────────────────

/// Record a canonical `needs` edge from `src` to `dst`.
///
/// Called from wrapper code via:
/// `stack::with_top(|src| registry::edge(src, resource_endpoint_id))`
pub(crate) fn edge(src: &str, dst: &str) {
    EDGES
        .lock()
        .unwrap()
        .insert((src.to_string(), dst.to_string()));
}

/// Remove a previously recorded edge.
///
/// Called when a resource is no longer being waited on (lock acquired,
/// message received, permits obtained, etc.).
pub(crate) fn remove_edge(src: &str, dst: &str) {
    EDGES
        .lock()
        .unwrap()
        .remove(&(src.to_string(), dst.to_string()));
}

/// Remove all edges originating from `src`.
///
/// Called when a future completes or is dropped, to clean up all
/// edges it may have emitted.
pub(crate) fn remove_edges_from(src: &str) {
    EDGES.lock().unwrap().retain(|(s, _)| s != src);
}

// ── Graph emission ───────────────────────────────────────

/// Emit the canonical graph snapshot for all tracked resources.
///
/// Combines:
/// - Process metadata from `init()`
/// - Canonical `needs` edges from stack-mediated interactions
/// - Resource-specific nodes and edges from each resource module
///
/// Resource-specific emission is added by migration tasks #2-#4 as
/// each resource module (futures, locks, sync) is moved into the crate.
pub(crate) fn emit_graph() -> GraphSnapshot {
    let Some(info) = PROCESS_INFO.get() else {
        return GraphSnapshot::default();
    };

    let canonical_edges: Vec<Edge> = EDGES
        .lock()
        .unwrap()
        .iter()
        .map(|(src, dst)| Edge {
            src: src.clone(),
            dst: dst.clone(),
            attrs_json: "{}".to_string(),
        })
        .collect();

    let mut graph = GraphSnapshot {
        process_name: info.name.clone(),
        proc_key: info.proc_key.clone(),
        nodes: Vec::new(),
        edges: canonical_edges,
    };

    // Collect nodes and edges from each resource module.
    crate::futures::emit_into_graph(&mut graph);

    graph
}
