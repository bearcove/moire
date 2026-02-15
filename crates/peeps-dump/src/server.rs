use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use peeps_types::{DashboardPayload, ProcessDump};
use peeps_waitgraph::detect::{self, Severity};
use peeps_waitgraph::{EdgeConfidence, EdgeKind, NodeId, NodeKind, WaitGraph};
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};

/// Key for identifying a process: (process_name, pid).
type ProcessKey = (String, u32);

/// Notification sent on each state update, carrying the new seq and which sections changed.
#[derive(Clone, Debug)]
pub struct UpdateNotification {
    pub seq: u64,
    pub changed: Vec<String>,
}

/// Shared dashboard state, holding the latest dump from each connected process.
pub struct DashboardState {
    dumps: Mutex<HashMap<ProcessKey, ProcessDump>>,
    notify: broadcast::Sender<UpdateNotification>,
    seq: AtomicU64,
}

impl DashboardState {
    pub fn new() -> Self {
        let (notify, _) = broadcast::channel(16);
        Self {
            dumps: Mutex::new(HashMap::new()),
            notify,
            seq: AtomicU64::new(0),
        }
    }

    /// Current sequence number.
    pub fn current_seq(&self) -> u64 {
        self.seq.load(Ordering::Relaxed)
    }

    /// Insert or update a dump. Notifies subscribers with seq and changed sections.
    pub async fn upsert_dump(&self, dump: ProcessDump) {
        let changed = sections_from_dump(&dump);
        let key = (dump.process_name.clone(), dump.pid);
        self.dumps.lock().await.insert(key, dump);
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.notify.send(UpdateNotification { seq, changed });
    }

    /// Get all current dumps as a sorted vec.
    pub async fn all_dumps(&self) -> Vec<ProcessDump> {
        let map = self.dumps.lock().await;
        let mut dumps: Vec<ProcessDump> = map.values().cloned().collect();
        dumps.sort_by(|a, b| a.process_name.cmp(&b.process_name));
        dumps
    }

    /// Build the full dashboard payload with dumps and deadlock candidates.
    pub async fn dashboard_payload(&self) -> DashboardPayload {
        let dumps = self.all_dumps().await;
        let graph = WaitGraph::build(&dumps);
        let raw_candidates = detect::find_deadlock_candidates(&graph);
        let deadlock_candidates = raw_candidates
            .into_iter()
            .enumerate()
            .map(|(i, c)| convert_candidate(i as u32, &c, &graph, &dumps))
            .collect();
        DashboardPayload {
            dumps,
            deadlock_candidates,
        }
    }

    /// Subscribe to change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<UpdateNotification> {
        self.notify.subscribe()
    }
}

/// Determine which dashboard sections a dump contributes data to.
fn sections_from_dump(dump: &ProcessDump) -> Vec<String> {
    let mut sections = Vec::new();
    if !dump.tasks.is_empty() {
        sections.push("tasks".to_string());
    }
    if !dump.threads.is_empty() {
        sections.push("threads".to_string());
    }
    if dump.locks.is_some() {
        sections.push("locks".to_string());
    }
    if dump.sync.is_some() {
        sections.push("sync".to_string());
    }
    if dump.roam.is_some() {
        sections.push("requests".to_string());
        sections.push("connections".to_string());
    }
    if dump.shm.is_some() {
        sections.push("shm".to_string());
    }
    // Every dump represents a process.
    sections.push("processes".to_string());
    sections
}

// ── Candidate conversion ─────────────────────────────────────────

fn convert_candidate(
    id: u32,
    candidate: &detect::DeadlockCandidate,
    graph: &WaitGraph,
    dumps: &[ProcessDump],
) -> peeps_types::DeadlockCandidate {
    let pid_to_name: HashMap<u32, &str> = dumps
        .iter()
        .map(|d| (d.pid, d.process_name.as_str()))
        .collect();

    // Build cycle_path nodes from the candidate's cycle_path (which closes: first == last).
    // We skip the closing duplicate node.
    let path_nodes: Vec<&NodeId> = if candidate.cycle_path.len() > 1 {
        candidate.cycle_path[..candidate.cycle_path.len() - 1]
            .iter()
            .collect()
    } else {
        candidate.cycle_path.iter().collect()
    };

    let cycle_path: Vec<peeps_types::CycleNode> = path_nodes
        .iter()
        .map(|node_id| node_id_to_cycle_node(node_id, graph, &pid_to_name))
        .collect();

    // Build edges between consecutive path nodes (and wrap around)
    let cycle_edges: Vec<peeps_types::CycleEdge> = if path_nodes.len() >= 2 {
        (0..path_nodes.len())
            .map(|i| {
                let from = i as u32;
                let to = ((i + 1) % path_nodes.len()) as u32;
                let from_id = path_nodes[i];
                let to_id = path_nodes[(i + 1) % path_nodes.len()];
                let (explanation, wait_secs, confidence) =
                    edge_explanation(from_id, to_id, graph, &cycle_path);
                peeps_types::CycleEdge {
                    from_node: from,
                    to_node: to,
                    explanation,
                    wait_secs,
                    confidence,
                }
            })
            .collect()
    } else {
        vec![]
    };

    let severity = match candidate.severity {
        Severity::Danger => peeps_types::DeadlockSeverity::Danger,
        Severity::Warn | Severity::Info => peeps_types::DeadlockSeverity::Warn,
    };

    let cross_process = {
        let mut pids = std::collections::BTreeSet::new();
        for node in &candidate.cycle_path {
            if let Some(pid) = node_pid(node) {
                pids.insert(pid);
            }
        }
        pids.len() > 1
    };

    let worst_wait_secs = candidate
        .edges
        .iter()
        .filter_map(|e| {
            match graph.nodes.get(&e.from) {
                Some(NodeKind::Task { age_secs, .. }) => Some(*age_secs),
                Some(NodeKind::RpcRequest { elapsed_secs, .. }) => Some(*elapsed_secs),
                _ => None,
            }
        })
        .fold(0.0_f64, f64::max);

    let title = build_title(&cycle_path, cross_process);

    peeps_types::DeadlockCandidate {
        id,
        severity,
        score: candidate.severity_score as f64,
        title,
        cycle_path,
        cycle_edges,
        rationale: candidate.rationale.clone(),
        cross_process,
        worst_wait_secs,
        blocked_task_count: count_blocked_outside(candidate, graph),
    }
}

fn node_pid(node: &NodeId) -> Option<u32> {
    match node {
        NodeId::Task { pid, .. }
        | NodeId::Future { pid, .. }
        | NodeId::Lock { pid, .. }
        | NodeId::MpscChannel { pid, .. }
        | NodeId::OneshotChannel { pid, .. }
        | NodeId::WatchChannel { pid, .. }
        | NodeId::Semaphore { pid, .. }
        | NodeId::RoamChannel { pid, .. }
        | NodeId::OnceCell { pid, .. }
        | NodeId::Socket { pid, .. }
        | NodeId::Unknown { pid, .. }
        | NodeId::RpcRequest { pid, .. }
        | NodeId::Process { pid } => Some(*pid),
    }
}

fn node_id_to_cycle_node(
    node_id: &NodeId,
    graph: &WaitGraph,
    pid_to_name: &HashMap<u32, &str>,
) -> peeps_types::CycleNode {
    let pid = node_pid(node_id).unwrap_or(0);
    let process = pid_to_name
        .get(&pid)
        .unwrap_or(&"unknown")
        .to_string();

    match graph.nodes.get(node_id) {
        Some(NodeKind::Task {
            name, state: _, age_secs: _,
        }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "task".to_string(),
            process,
            task_id: match node_id {
                NodeId::Task { task_id, .. } => Some(*task_id),
                _ => None,
            },
        },
        Some(NodeKind::Lock { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "lock".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::RpcRequest {
            method_name,
            direction: _,
            elapsed_secs: _,
        }) => peeps_types::CycleNode {
            label: method_name
                .clone()
                .unwrap_or_else(|| "rpc".to_string()),
            kind: "rpc".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::MpscChannel { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "channel".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::OneshotChannel { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "channel".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::WatchChannel { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "channel".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::Semaphore { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "semaphore".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::RoamChannel { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "roam-channel".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::Future { resource }) => peeps_types::CycleNode {
            label: resource.clone(),
            kind: "future".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::OnceCell { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "oncecell".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::Socket { fd, label, .. }) => peeps_types::CycleNode {
            label: label.clone().unwrap_or_else(|| format!("socket:{fd}")),
            kind: "socket".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::Unknown { label }) => peeps_types::CycleNode {
            label: label.clone(),
            kind: "unknown".to_string(),
            process,
            task_id: None,
        },
        Some(NodeKind::Process { name, .. }) => peeps_types::CycleNode {
            label: name.clone(),
            kind: "process".to_string(),
            process,
            task_id: None,
        },
        None => peeps_types::CycleNode {
            label: "unknown".to_string(),
            kind: "unknown".to_string(),
            process,
            task_id: None,
        },
    }
}

fn edge_explanation(
    from: &NodeId,
    to: &NodeId,
    graph: &WaitGraph,
    cycle_nodes: &[peeps_types::CycleNode],
) -> (String, f64, peeps_types::CycleEdgeConfidence) {
    // Find the matching edge in the graph
    for edge in &graph.edges {
        if edge.from == *from && edge.to == *to {
            let from_label = node_label(from, graph);
            let to_label = node_label(to, graph);
            let wait_secs = match &edge.kind {
                EdgeKind::TaskWaitsOnResource => match graph.nodes.get(from) {
                    Some(NodeKind::Task { age_secs, .. }) => *age_secs,
                    _ => 0.0,
                },
                EdgeKind::RpcClientToRequest => match graph.nodes.get(to) {
                    Some(NodeKind::RpcRequest { elapsed_secs, .. }) => *elapsed_secs,
                    _ => 0.0,
                },
                _ => 0.0,
            };
            let explanation = match &edge.kind {
                EdgeKind::TaskWaitsOnResource => {
                    format!("{from_label} waits on {to_label}")
                }
                EdgeKind::ResourceOwnedByTask => {
                    format!("{from_label} held by {to_label}")
                }
                EdgeKind::RpcClientToRequest => {
                    format!("{from_label} waiting on RPC {to_label}")
                }
                EdgeKind::RpcRequestToServerTask => {
                    format!("RPC {from_label} handled by {to_label}")
                }
                _ => format!("{from_label} -> {to_label}"),
            };
            let confidence = match edge.meta.confidence {
                EdgeConfidence::Explicit => peeps_types::CycleEdgeConfidence::Explicit,
                EdgeConfidence::Derived => peeps_types::CycleEdgeConfidence::Derived,
                EdgeConfidence::Heuristic => peeps_types::CycleEdgeConfidence::Heuristic,
            };
            return (explanation, wait_secs, confidence);
        }
    }
    let _ = cycle_nodes; // used for context in the signature
    ("unknown relationship".to_string(), 0.0, peeps_types::CycleEdgeConfidence::Derived)
}

fn node_label(node: &NodeId, graph: &WaitGraph) -> String {
    match graph.nodes.get(node) {
        Some(NodeKind::Task { name, .. }) => format!("task \"{name}\""),
        Some(NodeKind::Lock { name, .. }) => format!("lock \"{name}\""),
        Some(NodeKind::RpcRequest { method_name, .. }) => {
            format!("RPC \"{}\"", method_name.as_deref().unwrap_or("unknown"))
        }
        Some(NodeKind::MpscChannel { name, .. }) => format!("channel \"{name}\""),
        Some(NodeKind::OneshotChannel { name, .. }) => format!("oneshot \"{name}\""),
        Some(NodeKind::WatchChannel { name, .. }) => format!("watch \"{name}\""),
        Some(NodeKind::Semaphore { name, .. }) => format!("semaphore \"{name}\""),
        Some(NodeKind::OnceCell { name, .. }) => format!("oncecell \"{name}\""),
        Some(NodeKind::RoamChannel { name, .. }) => format!("roam-channel \"{name}\""),
        Some(NodeKind::Socket { fd, label, .. }) => {
            format!("socket \"{}\"", label.as_deref().unwrap_or(&format!("fd:{fd}")))
        }
        Some(NodeKind::Unknown { label }) => format!("resource \"{label}\""),
        Some(NodeKind::Future { resource }) => format!("future \"{resource}\""),
        Some(NodeKind::Process { name, .. }) => format!("process \"{name}\""),
        None => "unknown".to_string(),
    }
}

fn build_title(cycle_nodes: &[peeps_types::CycleNode], cross_process: bool) -> String {
    let task_names: Vec<&str> = cycle_nodes
        .iter()
        .filter(|n| n.kind == "task")
        .map(|n| n.label.as_str())
        .collect();

    let prefix = if cross_process {
        "Cross-process deadlock"
    } else {
        "Deadlock"
    };

    match task_names.len() {
        0 => format!("{prefix} involving {} nodes", cycle_nodes.len()),
        1 => format!("{prefix}: {}", task_names[0]),
        2 => format!("{prefix}: {} <-> {}", task_names[0], task_names[1]),
        n => format!(
            "{prefix}: {}, {}, and {} more",
            task_names[0],
            task_names[1],
            n - 2
        ),
    }
}

fn count_blocked_outside(
    candidate: &detect::DeadlockCandidate,
    graph: &WaitGraph,
) -> u32 {
    let mut blocked = std::collections::BTreeSet::new();
    for edge in &graph.edges {
        if edge.kind == EdgeKind::TaskWaitsOnResource && candidate.nodes.contains(&edge.to) {
            if !candidate.nodes.contains(&edge.from) {
                if matches!(edge.from, NodeId::Task { .. }) {
                    blocked.insert(edge.from.clone());
                }
            }
        }
    }
    blocked.len() as u32
}

/// Accept TCP connections and spawn a reader task for each.
pub async fn run_tcp_acceptor(listener: TcpListener, state: Arc<DashboardState>) {
    let max_frame_bytes = max_frame_bytes_from_env();
    eprintln!("[peeps] max frame size set to {max_frame_bytes} bytes");
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                eprintln!("[peeps] TCP connection from {addr}");
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle_tcp_connection(stream, &state, max_frame_bytes).await {
                        eprintln!("[peeps] connection from {addr} closed: {e}");
                    } else {
                        eprintln!("[peeps] connection from {addr} closed");
                    }
                });
            }
            Err(e) => {
                eprintln!("[peeps] TCP accept error: {e}");
            }
        }
    }
}

/// Read length-prefixed JSON frames from a single TCP connection.
///
/// Wire format: `[u32 big-endian length][UTF-8 JSON ProcessDump]`
async fn handle_tcp_connection(
    mut stream: TcpStream,
    state: &DashboardState,
    max_frame_bytes: usize,
) -> std::io::Result<()> {
    loop {
        // Read 4-byte length prefix (big-endian u32).
        let len = stream.read_u32().await?;

        if len == 0 {
            continue;
        }

        // Sanity limit to avoid unbounded memory growth on malformed clients.
        if (len as usize) > max_frame_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {len} bytes (max {max_frame_bytes})"),
            ));
        }

        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let json = match std::str::from_utf8(&buf) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[peeps] invalid UTF-8 in frame: {e}");
                continue;
            }
        };

        match facet_json::from_str::<ProcessDump>(json) {
            Ok(dump) => {
                eprintln!(
                    "[peeps] dump from {} (pid {}): {} tasks, {} threads",
                    dump.process_name,
                    dump.pid,
                    dump.tasks.len(),
                    dump.threads.len()
                );
                state.upsert_dump(dump).await;
            }
            Err(e) => {
                eprintln!("[peeps] failed to parse dump frame: {e}");
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn test_dump(name: &str, pid: u32) -> ProcessDump {
    use peeps_types::*;
    use std::collections::HashMap;

    ProcessDump {
        process_name: name.to_string(),
        pid,
        timestamp: "2026-02-15T00:00:00Z".to_string(),
        tasks: vec![TaskSnapshot {
            id: 1,
            name: "test-task".to_string(),
            state: TaskState::Pending,
            spawned_at_secs: 0.0,
            age_secs: 1.0,
            spawn_backtrace: String::new(),
            poll_events: vec![],
            parent_task_id: None,
            parent_task_name: None,
        }],
        wake_edges: vec![WakeEdgeSnapshot {
            source_task_id: Some(1),
            source_task_name: Some("src".to_string()),
            target_task_id: 2,
            target_task_name: Some("dst".to_string()),
            wake_count: 1,
            last_wake_age_secs: 0.5,
        }],
        future_wake_edges: vec![FutureWakeEdgeSnapshot {
            source_task_id: Some(1),
            source_task_name: Some("src".to_string()),
            future_id: 100,
            future_resource: "res".to_string(),
            target_task_id: Some(2),
            target_task_name: Some("dst".to_string()),
            wake_count: 1,
            last_wake_age_secs: 0.1,
        }],
        future_waits: vec![FutureWaitSnapshot {
            future_id: 100,
            task_id: 1,
            task_name: Some("test-task".to_string()),
            resource: "res".to_string(),
            created_by_task_id: None,
            created_by_task_name: None,
            created_age_secs: 0.0,
            last_polled_by_task_id: None,
            last_polled_by_task_name: None,
            pending_count: 1,
            ready_count: 0,
            total_pending_secs: 0.5,
            last_seen_age_secs: 0.1,
        }],
        threads: vec![ThreadStackSnapshot {
            name: "main".to_string(),
            backtrace: Some("frame0\nframe1".to_string()),
            samples: 10,
            responded: 10,
            same_location_count: 5,
            dominant_frame: Some("frame0".to_string()),
        }],
        locks: Some(LockSnapshot {
            locks: vec![LockInfoSnapshot {
                name: "my-lock".to_string(),
                acquires: 10,
                releases: 9,
                holders: vec![],
                waiters: vec![],
            }],
        }),
        sync: Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![],
            once_cells: vec![],
        }),
        roam: Some(SessionSnapshot {
            connections: vec![],
            method_names: HashMap::new(),
            channel_details: vec![],
        }),
        shm: Some(ShmSnapshot {
            segments: vec![],
            channels: vec![],
        }),
        future_spawn_edges: vec![],
        future_poll_edges: vec![],
        future_resume_edges: vec![],
        future_resource_edges: vec![],
        request_parents: vec![],
        custom: HashMap::new(),
    }
}

fn max_frame_bytes_from_env() -> usize {
    const DEFAULT_MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
    match std::env::var("PEEPS_MAX_FRAME_BYTES") {
        Ok(raw) => match raw.parse::<usize>() {
            Ok(v) if v > 0 => v,
            _ => {
                eprintln!(
                    "[peeps] invalid PEEPS_MAX_FRAME_BYTES={raw:?}, using default {}",
                    DEFAULT_MAX_FRAME_BYTES
                );
                DEFAULT_MAX_FRAME_BYTES
            }
        },
        Err(_) => DEFAULT_MAX_FRAME_BYTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_starts_at_zero() {
        let state = DashboardState::new();
        assert_eq!(state.current_seq(), 0);
    }

    #[tokio::test]
    async fn seq_increments_on_upsert() {
        let state = DashboardState::new();
        assert_eq!(state.current_seq(), 0);

        state.upsert_dump(test_dump("app", 1)).await;
        assert_eq!(state.current_seq(), 1);
    }

    #[tokio::test]
    async fn seq_monotonic_across_multiple_upserts() {
        let state = DashboardState::new();

        for i in 0..5 {
            state.upsert_dump(test_dump("app", i)).await;
        }
        assert_eq!(state.current_seq(), 5);
    }

    #[tokio::test]
    async fn subscribe_receives_update_notification() {
        let state = DashboardState::new();
        let mut rx = state.subscribe();

        state.upsert_dump(test_dump("app", 42)).await;

        let notification = rx.recv().await.expect("should receive notification");
        assert_eq!(notification.seq, 1);
        assert!(notification.changed.contains(&"tasks".to_string()));
        assert!(notification.changed.contains(&"threads".to_string()));
        assert!(notification.changed.contains(&"locks".to_string()));
        assert!(notification.changed.contains(&"processes".to_string()));
    }

    #[tokio::test]
    async fn all_dumps_sorted_by_name() {
        let state = DashboardState::new();
        state.upsert_dump(test_dump("zebra", 3)).await;
        state.upsert_dump(test_dump("alpha", 1)).await;
        state.upsert_dump(test_dump("middle", 2)).await;

        let dumps = state.all_dumps().await;
        assert_eq!(dumps.len(), 3);
        assert_eq!(dumps[0].process_name, "alpha");
        assert_eq!(dumps[1].process_name, "middle");
        assert_eq!(dumps[2].process_name, "zebra");
    }

    #[tokio::test]
    async fn upsert_replaces_same_process_key() {
        let state = DashboardState::new();
        state.upsert_dump(test_dump("app", 1)).await;
        state.upsert_dump(test_dump("app", 1)).await;

        let dumps = state.all_dumps().await;
        assert_eq!(dumps.len(), 1);
        assert_eq!(state.current_seq(), 2);
    }

    #[test]
    fn sections_from_full_dump() {
        let dump = test_dump("app", 1);
        let sections = sections_from_dump(&dump);
        assert!(sections.contains(&"tasks".to_string()));
        assert!(sections.contains(&"threads".to_string()));
        assert!(sections.contains(&"locks".to_string()));
        assert!(sections.contains(&"sync".to_string()));
        assert!(sections.contains(&"requests".to_string()));
        assert!(sections.contains(&"connections".to_string()));
        assert!(sections.contains(&"shm".to_string()));
        assert!(sections.contains(&"processes".to_string()));
    }
}
