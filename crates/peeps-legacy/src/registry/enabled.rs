use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use facet::Facet;
use peeps_types_legacy::{Edge, EdgeKind, Event, GraphSnapshot, Node};

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

// ── Spawn lineage edge storage ───────────────────────────
//
// Stores `spawned` edges: "src spawned dst". Permanent historical fact,
// retained for the lifetime of the child node.

static SPAWNED_EDGES: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

// ── External node storage ────────────────────────────────
//
// Stores nodes registered by external crates (e.g. roam registering
// request/response/channel nodes). These are included in emit_graph().

struct ExternalNodeEntry {
    node: Node,
    created_at: i64,
}

static EXTERNAL_NODES: LazyLock<Mutex<HashMap<String, ExternalNodeEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// ── Event storage ───────────────────────────────────────
//
// Runtime event retention is bounded with deterministic FIFO eviction.
// When capacity is reached, we always evict exactly one oldest event
// (`pop_front`) before appending the newest (`push_back`).
//
// This guarantees:
// - Memory is bounded (no unbounded growth).
// - Eviction order is deterministic.
// - Emission order in snapshots is oldest → newest.

const EVENT_BUFFER_CAPACITY: usize = 4096;

static EVENTS: LazyLock<Mutex<VecDeque<Event>>> = LazyLock::new(|| Mutex::new(VecDeque::new()));

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

fn now_unix_ns_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub fn created_at_now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}

pub fn make_node(
    id: impl Into<String>,
    kind: peeps_types_legacy::NodeKind,
    label: Option<String>,
    attrs_json: impl Into<String>,
    created_at: i64,
) -> Node {
    let canonical_created_at = if created_at > 0 {
        created_at
    } else {
        created_at_now_ns()
    };
    Node {
        id: id.into(),
        kind,
        label,
        attrs_json: attrs_with_created_at(attrs_json.into(), canonical_created_at),
    }
}

fn attrs_with_created_at(attrs_json: String, created_at: i64) -> String {
    #[derive(Facet)]
    struct SourceAttr {
        source: Option<String>,
    }

    let has_source = facet_json::from_slice::<SourceAttr>(attrs_json.as_bytes())
        .ok()
        .and_then(|attrs| attrs.source)
        .map(|source| !source.trim().is_empty())
        .unwrap_or(false);

    let trimmed = attrs_json.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return format!(r#"{{"created_at":{created_at},"source":"peeps/unknown"}}"#);
    }

    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return format!(r#"{{"created_at":{created_at},"source":"peeps/unknown"}}"#);
    }

    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        format!(r#"{{"created_at":{created_at},"source":"peeps/unknown"}}"#)
    } else if has_source {
        format!(r#"{{"created_at":{created_at},{inner}}}"#)
    } else {
        format!(r#"{{"created_at":{created_at},"source":"peeps/unknown",{inner}}}"#)
    }
}

pub fn record_event(entity_id: &str, name: &str, attrs_json: impl Into<String>) {
    let Some(info) = PROCESS_INFO.get() else {
        return;
    };

    let event = Event {
        id: peeps_types_legacy::new_node_id("event"),
        ts_ns: now_unix_ns_u64(),
        proc_key: info.proc_key.clone(),
        entity_id: entity_id.to_string(),
        name: name.to_string(),
        parent_entity_id: crate::stack::capture_top(),
        attrs_json: attrs_json.into(),
    };

    let mut events = EVENTS.lock().unwrap();
    if events.len() == EVENT_BUFFER_CAPACITY {
        events.pop_front();
    }
    events.push_back(event);
}

fn take_events() -> Option<Vec<Event>> {
    let mut events = EVENTS.lock().unwrap();
    if events.is_empty() {
        return None;
    }
    Some(events.drain(..).collect())
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

// ── Spawn edge tracking ─────────────────────────────────

/// Record a `spawned` edge from `src` to `dst`.
///
/// Indicates that `src` spawned `dst`. This is a permanent historical fact
/// retained for the lifetime of the child node.
pub fn spawn_edge(src: &str, dst: &str) {
    SPAWNED_EDGES
        .lock()
        .unwrap()
        .insert((src.to_string(), dst.to_string()));
}

/// Remove all spawn edges pointing to `dst`.
///
/// Called when the child node is dropped.
pub fn remove_spawn_edges_to(dst: &str) {
    SPAWNED_EDGES.lock().unwrap().retain(|(_, d)| d != dst);
}

// ── External node registration ──────────────────────────

/// Register a node in the global registry.
///
/// Used by external crates (e.g. roam) to register request/response/channel
/// nodes that should appear in the canonical graph.
pub fn register_node(node: Node) {
    let mut nodes = EXTERNAL_NODES.lock().unwrap();
    let node_id = node.id.clone();
    let incoming_attrs_json = node.attrs_json;
    let incoming_created_at = extract_created_at(&incoming_attrs_json);
    let raw_attrs_json = strip_created_at_prefix(incoming_attrs_json);
    if let Some(entry) = nodes.get_mut(&node_id) {
        entry.node = make_node(
            node.id,
            node.kind,
            node.label,
            raw_attrs_json,
            entry.created_at,
        );
        return;
    }

    let created_at = incoming_created_at.unwrap_or_else(created_at_now_ns);
    let canonical = make_node(node.id, node.kind, node.label, raw_attrs_json, created_at);
    nodes.insert(
        node_id,
        ExternalNodeEntry {
            created_at,
            node: canonical,
        },
    );
}

fn extract_created_at(attrs_json: &str) -> Option<i64> {
    #[derive(Facet)]
    struct CreatedAtAttrs {
        created_at: i64,
    }

    facet_json::from_slice::<CreatedAtAttrs>(attrs_json.as_bytes())
        .ok()
        .map(|attrs| attrs.created_at)
}

fn strip_created_at_prefix(attrs_json: String) -> String {
    let trimmed = attrs_json.trim();
    let prefix = r#"{"created_at":"#;
    if !trimmed.starts_with(prefix) {
        return attrs_json;
    }

    let bytes = trimmed.as_bytes();
    let mut idx = prefix.len();
    while idx < bytes.len() && (bytes[idx].is_ascii_digit() || bytes[idx] == b'-') {
        idx += 1;
    }
    if idx >= bytes.len() {
        return attrs_json;
    }

    match bytes[idx] {
        b'}' => "{}".to_string(),
        b',' => {
            let rest = &trimmed[idx + 1..trimmed.len() - 1];
            format!("{{{rest}}}")
        }
        _ => attrs_json,
    }
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
    SPAWNED_EDGES
        .lock()
        .unwrap()
        .retain(|(s, d)| s != id && d != id);
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

    canonical_edges.extend(SPAWNED_EDGES.lock().unwrap().iter().map(|(src, dst)| Edge {
        src: src.clone(),
        dst: dst.clone(),
        kind: EdgeKind::Spawned,
        attrs_json: "{}".to_string(),
    }));

    let external_nodes: Vec<Node> = EXTERNAL_NODES
        .lock()
        .unwrap()
        .values()
        .map(|entry| entry.node.clone())
        .collect();

    let mut graph = GraphSnapshot {
        process_name: info.name.clone(),
        proc_key: info.proc_key.clone(),
        nodes: external_nodes,
        edges: canonical_edges,
        events: take_events(),
    };

    // Collect nodes and edges from each resource module.
    crate::futures::emit_into_graph(&mut graph);
    crate::locks::emit_into_graph(&mut graph);
    crate::sync::emit_into_graph(&mut graph);
    enforce_created_at_invariant(&graph.nodes);

    let mut needs = 0u32;
    let mut touches = 0u32;
    let mut spawned = 0u32;
    let mut closed_by = 0u32;
    for e in &graph.edges {
        match e.kind {
            EdgeKind::Needs => needs += 1,
            EdgeKind::Touches => touches += 1,
            EdgeKind::Spawned => spawned += 1,
            EdgeKind::ClosedBy => closed_by += 1,
        }
    }

    let mut futures = 0u32;
    let mut locks = 0u32;
    let mut tx = 0u32;
    let mut rx = 0u32;
    let mut remote_tx = 0u32;
    let mut remote_rx = 0u32;
    let mut requests = 0u32;
    let mut responses = 0u32;
    let mut connections = 0u32;
    let mut join_sets = 0u32;
    let mut semaphores = 0u32;
    let mut once_cells = 0u32;
    let mut commands = 0u32;
    let mut file_ops = 0u32;
    let mut notifies = 0u32;
    let mut sleeps = 0u32;
    let mut intervals = 0u32;
    let mut timeouts = 0u32;
    let mut net_connects = 0u32;
    let mut net_accepts = 0u32;
    let mut net_readables = 0u32;
    let mut net_writables = 0u32;
    let mut syscalls = 0u32;
    for n in &graph.nodes {
        match n.kind {
            peeps_types_legacy::NodeKind::Future => futures += 1,
            peeps_types_legacy::NodeKind::Lock => locks += 1,
            peeps_types_legacy::NodeKind::Tx => tx += 1,
            peeps_types_legacy::NodeKind::Rx => rx += 1,
            peeps_types_legacy::NodeKind::RemoteTx => remote_tx += 1,
            peeps_types_legacy::NodeKind::RemoteRx => remote_rx += 1,
            peeps_types_legacy::NodeKind::Request => requests += 1,
            peeps_types_legacy::NodeKind::Response => responses += 1,
            peeps_types_legacy::NodeKind::Connection => connections += 1,
            peeps_types_legacy::NodeKind::JoinSet => join_sets += 1,
            peeps_types_legacy::NodeKind::Semaphore => semaphores += 1,
            peeps_types_legacy::NodeKind::OnceCell => once_cells += 1,
            peeps_types_legacy::NodeKind::Command => commands += 1,
            peeps_types_legacy::NodeKind::FileOp => file_ops += 1,
            peeps_types_legacy::NodeKind::Notify => notifies += 1,
            peeps_types_legacy::NodeKind::Sleep => sleeps += 1,
            peeps_types_legacy::NodeKind::Interval => intervals += 1,
            peeps_types_legacy::NodeKind::Timeout => timeouts += 1,
            peeps_types_legacy::NodeKind::NetConnect => net_connects += 1,
            peeps_types_legacy::NodeKind::NetAccept => net_accepts += 1,
            peeps_types_legacy::NodeKind::NetReadable => net_readables += 1,
            peeps_types_legacy::NodeKind::NetWritable => net_writables += 1,
            peeps_types_legacy::NodeKind::Syscall => syscalls += 1,
        }
    }

    tracing::warn!(
        needs,
        touches,
        spawned,
        closed_by,
        futures,
        locks,
        tx,
        rx,
        remote_tx,
        remote_rx,
        requests,
        responses,
        connections,
        join_sets,
        semaphores,
        once_cells,
        commands,
        file_ops,
        notifies,
        sleeps,
        intervals,
        timeouts,
        net_connects,
        net_accepts,
        net_readables,
        net_writables,
        syscalls,
        events = graph.events.as_ref().map(|v| v.len()).unwrap_or(0),
        nodes = graph.nodes.len(),
        edges = graph.edges.len(),
        "emit_graph completed"
    );

    graph
}

fn enforce_created_at_invariant(nodes: &[Node]) {
    #[derive(Facet)]
    struct CreatedAtAttrs {
        created_at: i64,
    }

    for node in nodes {
        let attrs = facet_json::from_slice::<CreatedAtAttrs>(node.attrs_json.as_bytes())
            .unwrap_or_else(|_| {
                panic!(
                    "node {} ({}) attrs_json missing parseable created_at",
                    node.id,
                    node.kind.as_str()
                )
            });
        assert!(
            attrs.created_at > 0,
            "node {} ({}) missing canonical created_at",
            node.id,
            node.kind.as_str()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peeps_types_legacy::NodeKind;

    fn extract_source(attrs_json: &str) -> Option<String> {
        #[derive(Facet)]
        struct SourceAttrs {
            source: String,
        }

        facet_json::from_slice::<SourceAttrs>(attrs_json.as_bytes())
            .ok()
            .map(|attrs| attrs.source)
    }

    fn reset_registry_state_for_test() {
        EXTERNAL_NODES.lock().unwrap().clear();
        EDGES.lock().unwrap().clear();
        TOUCH_EDGES.lock().unwrap().clear();
        SPAWNED_EDGES.lock().unwrap().clear();
        EVENTS.lock().unwrap().clear();
    }

    fn all_node_kinds() -> [NodeKind; 23] {
        [
            NodeKind::Future,
            NodeKind::Lock,
            NodeKind::Tx,
            NodeKind::Rx,
            NodeKind::RemoteTx,
            NodeKind::RemoteRx,
            NodeKind::Request,
            NodeKind::Response,
            NodeKind::Connection,
            NodeKind::JoinSet,
            NodeKind::Semaphore,
            NodeKind::OnceCell,
            NodeKind::Command,
            NodeKind::FileOp,
            NodeKind::Notify,
            NodeKind::Sleep,
            NodeKind::Interval,
            NodeKind::Timeout,
            NodeKind::NetConnect,
            NodeKind::NetAccept,
            NodeKind::NetReadable,
            NodeKind::NetWritable,
            NodeKind::Syscall,
        ]
    }

    #[test]
    fn register_node_preserves_first_created_at_for_same_id() {
        init("test-process", "test-proc-key");
        reset_registry_state_for_test();

        register_node(Node {
            id: "node:stable".to_string(),
            kind: NodeKind::Command,
            label: Some("first".to_string()),
            attrs_json: r#"{"created_at":111}"#.to_string(),
        });
        register_node(Node {
            id: "node:stable".to_string(),
            kind: NodeKind::Command,
            label: Some("second".to_string()),
            attrs_json: r#"{"created_at":999}"#.to_string(),
        });

        let graph = emit_graph();
        let node = graph
            .nodes
            .iter()
            .find(|n| n.id == "node:stable")
            .expect("node should exist");
        assert_eq!(extract_created_at(&node.attrs_json), Some(111));
    }

    #[test]
    fn all_node_kinds_emit_with_created_at() {
        init("test-process", "test-proc-key");
        reset_registry_state_for_test();

        for kind in all_node_kinds() {
            register_node(make_node(
                format!("{}:test", kind.as_str()),
                kind,
                Some(kind.as_str().to_string()),
                "{}",
                0,
            ));
        }

        let graph = emit_graph();
        assert_eq!(graph.nodes.len(), all_node_kinds().len());
        for kind in all_node_kinds() {
            let node = graph
                .nodes
                .iter()
                .find(|n| n.kind == kind)
                .unwrap_or_else(|| panic!("missing node kind {}", kind.as_str()));
            assert!(
                extract_created_at(&node.attrs_json).unwrap_or_default() > 0,
                "node kind {} missing created_at",
                kind.as_str()
            );
            assert!(
                !extract_source(&node.attrs_json)
                    .unwrap_or_default()
                    .trim()
                    .is_empty(),
                "node kind {} missing source",
                kind.as_str()
            );
        }
    }

    #[test]
    #[should_panic(expected = "missing parseable created_at")]
    fn invariant_panics_when_created_at_missing() {
        enforce_created_at_invariant(&[Node {
            id: "bad:node".to_string(),
            kind: NodeKind::Future,
            label: None,
            attrs_json: "{}".to_string(),
        }]);
    }
}
