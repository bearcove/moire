use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Instant;

use peeps_types::{Edge, EdgeKind, GraphSnapshot, Node};

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

// ── Historical interaction edge storage ──────────────────
//
// Stores `touches` edges: "src has interacted with dst at least once".
// Retained until either endpoint disappears.

static TOUCH_EDGES: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

// ── External node storage ────────────────────────────────
//
// Stores nodes registered by external crates (e.g. roam registering
// request/response/channel nodes). These are included in emit_graph().

struct ExternalNodeEntry {
    node: Node,
    created_at: Instant,
}

static EXTERNAL_NODES: LazyLock<Mutex<HashMap<String, ExternalNodeEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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
pub fn edge(src: &str, dst: &str) {
    EDGES
        .lock()
        .unwrap()
        .insert((src.to_string(), dst.to_string()));
}

/// Remove a previously recorded edge.
///
/// Called when a resource is no longer being waited on (lock acquired,
/// message received, permits obtained, etc.).
pub fn remove_edge(src: &str, dst: &str) {
    EDGES
        .lock()
        .unwrap()
        .remove(&(src.to_string(), dst.to_string()));
}

/// Remove all edges originating from `src`.
///
/// Called when a future completes or is dropped, to clean up all
/// edges it may have emitted.
pub fn remove_edges_from(src: &str) {
    EDGES.lock().unwrap().retain(|(s, _)| s != src);
}

/// Remove all edges pointing to `dst`.
///
/// Called when a node is removed, to clean up all edges targeting it.
pub fn remove_edges_to(dst: &str) {
    EDGES.lock().unwrap().retain(|(_, d)| d != dst);
}

// ── Touch edge tracking ─────────────────────────────────

/// Record a `touches` edge from `src` to `dst`.
///
/// Indicates that `src` has interacted with `dst` at least once.
/// The edge is retained until either endpoint disappears.
/// Deduplicates: calling this multiple times is a no-op.
pub fn touch_edge(src: &str, dst: &str) {
    TOUCH_EDGES
        .lock()
        .unwrap()
        .insert((src.to_string(), dst.to_string()));
}

/// Remove a previously recorded touch edge.
pub fn remove_touch_edge(src: &str, dst: &str) {
    TOUCH_EDGES
        .lock()
        .unwrap()
        .remove(&(src.to_string(), dst.to_string()));
}

/// Remove all touch edges originating from `src`.
pub fn remove_touch_edges_from(src: &str) {
    TOUCH_EDGES.lock().unwrap().retain(|(s, _)| s != src);
}

/// Remove all touch edges pointing to `dst`.
pub fn remove_touch_edges_to(dst: &str) {
    TOUCH_EDGES.lock().unwrap().retain(|(_, d)| d != dst);
}

// ── External node registration ──────────────────────────

/// Register a node in the global registry.
///
/// Used by external crates (e.g. roam) to register request/response/channel
/// nodes that should appear in the canonical graph.
pub fn register_node(node: Node) {
    let mut nodes = EXTERNAL_NODES.lock().unwrap();
    nodes
        .entry(node.id.clone())
        .and_modify(|entry| entry.node = node.clone())
        .or_insert_with(|| ExternalNodeEntry {
            node,
            created_at: Instant::now(),
        });
}

/// Remove a node from the global registry.
///
/// Also removes all edges (needs and touches) to/from this node.
pub fn remove_node(id: &str) {
    EXTERNAL_NODES.lock().unwrap().remove(id);
    EDGES.lock().unwrap().retain(|(s, d)| s != id && d != id);
    TOUCH_EDGES
        .lock()
        .unwrap()
        .retain(|(s, d)| s != id && d != id);
}

fn inject_elapsed_ns(attrs_json: &str, elapsed_ns: u64) -> String {
    if attrs_json.contains("\"elapsed_ns\"") {
        return attrs_json.to_string();
    }

    let trimmed = attrs_json.trim();
    if trimmed == "{}" {
        return format!("{{\"elapsed_ns\":{elapsed_ns}}}");
    }
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return attrs_json.to_string();
    }

    // Insert before trailing `}`.
    // Assumes attrs_json is valid JSON object (as required by Node.attrs_json).
    let insert_at = attrs_json.rfind('}').unwrap_or(attrs_json.len());
    let (head, tail) = attrs_json.split_at(insert_at);
    let needs_comma = head
        .chars()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| c != '{');
    if needs_comma {
        format!("{head},\"elapsed_ns\":{elapsed_ns}{tail}")
    } else {
        format!("{head}\"elapsed_ns\":{elapsed_ns}{tail}")
    }
}

// ── Graph emission ───────────────────────────────────────

/// Emit the canonical graph snapshot for all tracked resources.
///
/// Combines:
/// - Process metadata from `init()`
/// - Canonical `needs` edges from stack-mediated interactions
/// - Externally registered nodes (from `register_node()`)
/// - Resource-specific nodes and edges from each resource module
pub(crate) fn emit_graph() -> GraphSnapshot {
    let Some(info) = PROCESS_INFO.get() else {
        return GraphSnapshot::default();
    };

    let now = Instant::now();

    let mut canonical_edges: Vec<Edge> = EDGES
        .lock()
        .unwrap()
        .iter()
        .map(|(src, dst)| Edge {
            src: src.clone(),
            dst: dst.clone(),
            kind: EdgeKind::Needs,
            attrs_json: "{}".to_string(),
        })
        .collect();

    canonical_edges.extend(TOUCH_EDGES.lock().unwrap().iter().map(|(src, dst)| Edge {
        src: src.clone(),
        dst: dst.clone(),
        kind: EdgeKind::Touches,
        attrs_json: "{}".to_string(),
    }));

    let external_nodes: Vec<Node> = EXTERNAL_NODES
        .lock()
        .unwrap()
        .values()
        .map(|entry| {
            let mut node = entry.node.clone();
            if matches!(node.kind, peeps_types::NodeKind::Request | peeps_types::NodeKind::Response)
            {
                let elapsed_ns =
                    (now.duration_since(entry.created_at).as_nanos().min(u64::MAX as u128)) as u64;
                node.attrs_json = inject_elapsed_ns(&node.attrs_json, elapsed_ns);
            }
            node
        })
        .collect();

    let mut graph = GraphSnapshot {
        process_name: info.name.clone(),
        proc_key: info.proc_key.clone(),
        nodes: external_nodes,
        edges: canonical_edges,
    };

    // Collect nodes and edges from each resource module.
    crate::futures::emit_into_graph(&mut graph);
    crate::locks::emit_into_graph(&mut graph);
    crate::sync::emit_into_graph(&mut graph);

    graph
}

#[cfg(test)]
mod tests {
    use super::inject_elapsed_ns;

    #[test]
    fn inject_elapsed_ns_empty_object() {
        assert_eq!(inject_elapsed_ns("{}", 123), "{\"elapsed_ns\":123}");
    }

    #[test]
    fn inject_elapsed_ns_inserts_with_comma() {
        assert_eq!(inject_elapsed_ns("{\"a\":1}", 9), "{\"a\":1,\"elapsed_ns\":9}");
    }

    #[test]
    fn inject_elapsed_ns_inserts_without_comma_when_whitespace() {
        assert_eq!(
            inject_elapsed_ns("{  }", 42),
            "{  \"elapsed_ns\":42}"
        );
    }

    #[test]
    fn inject_elapsed_ns_noop_if_present() {
        assert_eq!(
            inject_elapsed_ns("{\"elapsed_ns\":1,\"a\":2}", 9),
            "{\"elapsed_ns\":1,\"a\":2}"
        );
    }

    #[test]
    fn inject_elapsed_ns_noop_if_not_object() {
        assert_eq!(inject_elapsed_ns("[]", 9), "[]");
        assert_eq!(inject_elapsed_ns("nope", 9), "nope");
    }
}
