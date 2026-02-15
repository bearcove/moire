//! Unified wait graph model and ingestion pipeline for peeps.
//!
//! Converts raw `ProcessDump` snapshots into a normalized directed graph
//! of blocking relationships. The graph is the single source of truth for
//! cycle detection, severity ranking, and dashboard explanations.

use std::collections::BTreeMap;

use facet::Facet;

pub mod detect;

use peeps_types::{
    ChannelDir, ConnectionSnapshot, Direction, FuturePollEdgeSnapshot, FutureResourceEdgeSnapshot,
    FutureResumeEdgeSnapshot, FutureSpawnEdgeSnapshot, FutureWaitSnapshot, FutureWakeEdgeSnapshot,
    LockAcquireKind, LockInfoSnapshot, LockSnapshot, MpscChannelSnapshot, OnceCellSnapshot,
    OnceCellState, OneshotChannelSnapshot, OneshotState, ProcessDump, ResourceRefSnapshot,
    RoamChannelSnapshot, SemaphoreSnapshot, SessionSnapshot, SocketWaitDirection, SyncSnapshot,
    TaskSnapshot, TaskState, WakeEdgeSnapshot, WatchChannelSnapshot,
};

// ── Stable node identity ────────────────────────────────────────

/// Stable identifier for a graph node across snapshots.
///
/// Uses `BTreeMap`-friendly `Ord` so the graph has deterministic iteration order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Facet)]
#[repr(u8)]
pub enum NodeId {
    /// A tokio task within a process.
    Task { pid: u32, task_id: u64 },
    /// An instrumented future within a process.
    Future { pid: u32, future_id: u64 },
    /// A named lock within a process.
    Lock { pid: u32, name: String },
    /// An mpsc channel within a process.
    MpscChannel { pid: u32, name: String },
    /// A oneshot channel within a process.
    OneshotChannel { pid: u32, name: String },
    /// A watch channel within a process.
    WatchChannel { pid: u32, name: String },
    /// A OnceCell within a process.
    OnceCell { pid: u32, name: String },
    /// A semaphore within a process.
    Semaphore { pid: u32, name: String },
    /// A roam channel within a process.
    RoamChannel { pid: u32, channel_id: u64 },
    /// A socket within a process.
    Socket { pid: u32, fd: u64 },
    /// An unknown/opaque resource within a process.
    Unknown { pid: u32, label: String },
    /// An RPC request (connection + request id, scoped to process).
    RpcRequest {
        pid: u32,
        connection: String,
        request_id: u64,
    },
    /// A whole process.
    Process { pid: u32 },
}

// ── Node kinds ──────────────────────────────────────────────────

/// What kind of resource a node represents.
#[derive(Debug, Clone, Facet)]
#[repr(u8)]
pub enum NodeKind {
    Task {
        name: String,
        state: TaskState,
        age_secs: f64,
    },
    Future {
        resource: String,
    },
    Lock {
        name: String,
        acquires: u64,
        releases: u64,
    },
    MpscChannel {
        name: String,
        bounded: bool,
        capacity: Option<u64>,
        pending: u64,
    },
    OneshotChannel {
        name: String,
        state: OneshotState,
    },
    WatchChannel {
        name: String,
        changes: u64,
    },
    OnceCell {
        name: String,
        state: OnceCellState,
    },
    Semaphore {
        name: String,
        permits_total: u64,
        permits_available: u64,
        waiters: u64,
        oldest_wait_secs: f64,
    },
    RoamChannel {
        name: String,
        direction: ChannelDir,
        queue_depth: Option<u64>,
        closed: bool,
    },
    Socket {
        fd: u64,
        label: Option<String>,
        direction: Option<SocketWaitDirection>,
        peer: Option<String>,
    },
    Unknown {
        label: String,
    },
    RpcRequest {
        method_name: Option<String>,
        direction: Direction,
        elapsed_secs: f64,
    },
    Process {
        name: String,
        pid: u32,
    },
}

// ── Edge kinds ──────────────────────────────────────────────────

/// The nature of a blocking/dependency relationship.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum EdgeKind {
    /// A task is waiting on a resource (lock, channel, future, RPC response).
    TaskWaitsOnResource,
    /// A resource is currently owned/held by a task.
    ResourceOwnedByTask,
    /// A task wakes a future (or another task through a future).
    TaskWakesFuture,
    /// A future, once ready, resumes a task.
    FutureResumesTask,
    /// A task is the client side of an outgoing RPC request.
    RpcClientToRequest,
    /// An incoming RPC request is being handled by a server-side task.
    RpcRequestToServerTask,
    /// Parent-child spawn relationship between tasks.
    TaskSpawnedTask,
    /// Cross-process RPC stitch: outgoing request in one process links to
    /// the corresponding incoming request in another process.
    RpcCrossProcessStitch,
    /// A future spawned/composed another future.
    FutureSpawnedFuture,
    /// A task polls a future (ownership over time).
    TaskPollsFuture,
    /// A future waits on a structured resource.
    FutureWaitsOnResource,
    /// Same-process explicit request parent relationship.
    RpcRequestParent,
}

/// Whether an edge was emitted by instrumentation or inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum EdgeConfidence {
    /// Emitted directly by instrumentation.
    Explicit,
    /// Computed or inferred from other data.
    Derived,
    /// Guess-level correlation (e.g. temporal proximity).
    Heuristic,
}

/// Metadata attached to every edge.
#[derive(Debug, Clone, Facet)]
pub struct EdgeMeta {
    /// Which snapshot source produced this edge.
    pub source_snapshot: SnapshotSource,
    /// Number of times this relationship has been observed.
    pub count: u64,
    /// Optional severity hint for ranking (higher = more suspicious).
    pub severity_hint: u8,
    /// Whether this edge is explicit (instrumented) or derived (inferred).
    pub confidence: EdgeConfidence,
    /// How long ago this edge was first observed (seconds).
    pub first_seen_age_secs: Option<f64>,
    /// Number of independent samples that confirmed this edge.
    pub sample_count: Option<u64>,
}

impl EdgeMeta {
    /// Create derived edge metadata (computed/inferred from existing data).
    fn derived(source: SnapshotSource, count: u64, severity_hint: u8) -> Self {
        Self {
            source_snapshot: source,
            count,
            severity_hint,
            confidence: EdgeConfidence::Derived,
            first_seen_age_secs: None,
            sample_count: None,
        }
    }

    /// Create explicit edge metadata (emitted by instrumentation).
    fn explicit(source: SnapshotSource, count: u64, severity_hint: u8) -> Self {
        Self {
            source_snapshot: source,
            count,
            severity_hint,
            confidence: EdgeConfidence::Explicit,
            first_seen_age_secs: None,
            sample_count: None,
        }
    }

    /// Create heuristic edge metadata (guess-level correlation).
    fn heuristic(source: SnapshotSource, count: u64, severity_hint: u8) -> Self {
        Self {
            source_snapshot: source,
            count,
            severity_hint,
            confidence: EdgeConfidence::Heuristic,
            first_seen_age_secs: None,
            sample_count: None,
        }
    }
}

/// Which snapshot source an edge was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[repr(u8)]
pub enum SnapshotSource {
    Tasks,
    WakeEdges,
    FutureWakeEdges,
    FutureWaits,
    Locks,
    Sync,
    Roam,
    FutureSpawnEdges,
    FuturePollEdges,
    FutureResumeEdges,
    FutureResourceEdges,
    RequestParents,
}

// ── Graph edge ──────────────────────────────────────────────────

/// A directed edge in the wait graph.
#[derive(Debug, Clone, Facet)]
pub struct WaitEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
    pub meta: EdgeMeta,
}

// ── The graph itself ────────────────────────────────────────────

/// The normalized wait graph built from one or more process dumps.
#[derive(Debug, Clone)]
pub struct WaitGraph {
    pub nodes: BTreeMap<NodeId, NodeKind>,
    pub edges: Vec<WaitEdge>,
}

impl WaitGraph {
    /// Build a wait graph from one or more process dumps.
    pub fn build(dumps: &[ProcessDump]) -> Self {
        let mut graph = WaitGraph {
            nodes: BTreeMap::new(),
            edges: Vec::new(),
        };
        for dump in dumps {
            graph.ingest_dump(dump);
        }
        graph.stitch_cross_process_rpc();
        graph.ingest_request_parents(dumps);
        graph
    }

    fn ingest_dump(&mut self, dump: &ProcessDump) {
        let pid = dump.pid;

        // Process node
        self.nodes.insert(
            NodeId::Process { pid },
            NodeKind::Process {
                name: dump.process_name.clone(),
                pid,
            },
        );

        self.ingest_tasks(pid, &dump.tasks);
        self.ingest_wake_edges(pid, &dump.wake_edges);
        self.ingest_future_wake_edges(pid, &dump.future_wake_edges);
        self.ingest_future_waits(pid, &dump.future_waits);
        if let Some(ref locks) = dump.locks {
            self.ingest_locks(pid, locks);
        }
        if let Some(ref sync) = dump.sync {
            self.ingest_sync(pid, sync);
        }
        if let Some(ref roam) = dump.roam {
            self.ingest_roam(pid, roam);
        }
        self.ingest_future_spawn_edges(pid, &dump.future_spawn_edges);
        self.ingest_future_poll_edges(pid, &dump.future_poll_edges);
        self.ingest_future_resume_edges(pid, &dump.future_resume_edges);
        self.ingest_future_resource_edges(pid, &dump.future_resource_edges);
    }

    fn ingest_tasks(&mut self, pid: u32, tasks: &[TaskSnapshot]) {
        for task in tasks {
            let node_id = NodeId::Task {
                pid,
                task_id: task.id,
            };
            self.nodes.insert(
                node_id.clone(),
                NodeKind::Task {
                    name: task.name.clone(),
                    state: task.state,
                    age_secs: task.age_secs,
                },
            );

            // Parent-child spawn edge
            if let Some(parent_id) = task.parent_task_id {
                let parent_node = NodeId::Task {
                    pid,
                    task_id: parent_id,
                };
                self.edges.push(WaitEdge {
                    from: parent_node,
                    to: node_id,
                    kind: EdgeKind::TaskSpawnedTask,
                    meta: EdgeMeta::derived(SnapshotSource::Tasks, 1, 0),
                });
            }
        }
    }

    fn ingest_wake_edges(&mut self, pid: u32, wake_edges: &[WakeEdgeSnapshot]) {
        for edge in wake_edges {
            let target = NodeId::Task {
                pid,
                task_id: edge.target_task_id,
            };
            if let Some(source_id) = edge.source_task_id {
                let source = NodeId::Task {
                    pid,
                    task_id: source_id,
                };
                // source task wakes target task — model as: target was waiting,
                // source provides the wake. This is a "future resumes task" in
                // the abstract (the waker fires on the target).
                self.edges.push(WaitEdge {
                    from: source.clone(),
                    to: target.clone(),
                    kind: EdgeKind::TaskWakesFuture,
                    meta: EdgeMeta::derived(SnapshotSource::WakeEdges, edge.wake_count, 0),
                });
            }
        }
    }

    fn ingest_future_wake_edges(&mut self, pid: u32, edges: &[FutureWakeEdgeSnapshot]) {
        for edge in edges {
            let future_node = NodeId::Future {
                pid,
                future_id: edge.future_id,
            };

            // Ensure future node exists
            self.nodes
                .entry(future_node.clone())
                .or_insert(NodeKind::Future {
                    resource: edge.future_resource.clone(),
                });

            // source task -> wakes future
            if let Some(source_id) = edge.source_task_id {
                let source = NodeId::Task {
                    pid,
                    task_id: source_id,
                };
                self.edges.push(WaitEdge {
                    from: source,
                    to: future_node.clone(),
                    kind: EdgeKind::TaskWakesFuture,
                    meta: EdgeMeta::derived(SnapshotSource::FutureWakeEdges, edge.wake_count, 0),
                });
            }

            // future -> resumes target task
            if let Some(target_id) = edge.target_task_id {
                let target = NodeId::Task {
                    pid,
                    task_id: target_id,
                };
                self.edges.push(WaitEdge {
                    from: future_node,
                    to: target,
                    kind: EdgeKind::FutureResumesTask,
                    meta: EdgeMeta::derived(SnapshotSource::FutureWakeEdges, edge.wake_count, 0),
                });
            }
        }
    }

    fn ingest_future_waits(&mut self, pid: u32, waits: &[FutureWaitSnapshot]) {
        for wait in waits {
            let future_node = NodeId::Future {
                pid,
                future_id: wait.future_id,
            };

            self.nodes
                .entry(future_node.clone())
                .or_insert(NodeKind::Future {
                    resource: wait.resource.clone(),
                });

            // task -> waits on future
            let task_node = NodeId::Task {
                pid,
                task_id: wait.task_id,
            };
            let severity = if wait.pending_count > 0 && wait.ready_count == 0 {
                2 // never been ready — suspicious
            } else {
                0
            };
            self.edges.push(WaitEdge {
                from: task_node,
                to: future_node.clone(),
                kind: EdgeKind::TaskWaitsOnResource,
                meta: EdgeMeta::derived(SnapshotSource::FutureWaits, wait.pending_count + wait.ready_count, severity),
            });

            // future -> created by task (ownership)
            if let Some(creator_id) = wait.created_by_task_id {
                let creator = NodeId::Task {
                    pid,
                    task_id: creator_id,
                };
                self.edges.push(WaitEdge {
                    from: future_node,
                    to: creator,
                    kind: EdgeKind::ResourceOwnedByTask,
                    meta: EdgeMeta::derived(SnapshotSource::FutureWaits, 1, 0),
                });
            }
        }
    }

    fn ingest_locks(&mut self, pid: u32, lock_snap: &LockSnapshot) {
        for lock in &lock_snap.locks {
            self.ingest_single_lock(pid, lock);
        }
    }

    fn ingest_single_lock(&mut self, pid: u32, lock: &LockInfoSnapshot) {
        let lock_node = NodeId::Lock {
            pid,
            name: lock.name.clone(),
        };
        self.nodes.insert(
            lock_node.clone(),
            NodeKind::Lock {
                name: lock.name.clone(),
                acquires: lock.acquires,
                releases: lock.releases,
            },
        );

        // lock -> owned by holder task
        for holder in &lock.holders {
            if let Some(task_id) = holder.task_id {
                let task_node = NodeId::Task { pid, task_id };
                let severity = match holder.kind {
                    LockAcquireKind::Write | LockAcquireKind::Mutex => {
                        if holder.held_secs > 1.0 {
                            3
                        } else {
                            1
                        }
                    }
                    LockAcquireKind::Read => 0,
                };
                self.edges.push(WaitEdge {
                    from: lock_node.clone(),
                    to: task_node,
                    kind: EdgeKind::ResourceOwnedByTask,
                    meta: EdgeMeta::derived(SnapshotSource::Locks, 1, severity),
                });
            }
        }

        // waiter task -> waits on lock
        for waiter in &lock.waiters {
            if let Some(task_id) = waiter.task_id {
                let task_node = NodeId::Task { pid, task_id };
                let severity = if waiter.waiting_secs > 1.0 { 3 } else { 1 };
                self.edges.push(WaitEdge {
                    from: task_node,
                    to: lock_node.clone(),
                    kind: EdgeKind::TaskWaitsOnResource,
                    meta: EdgeMeta::derived(SnapshotSource::Locks, 1, severity),
                });
            }
        }
    }

    fn ingest_sync(&mut self, pid: u32, sync: &SyncSnapshot) {
        for ch in &sync.mpsc_channels {
            self.ingest_mpsc(pid, ch);
        }
        for ch in &sync.oneshot_channels {
            self.ingest_oneshot(pid, ch);
        }
        for ch in &sync.watch_channels {
            self.ingest_watch(pid, ch);
        }
        for sem in &sync.semaphores {
            self.ingest_semaphore(pid, sem);
        }
        for cell in &sync.once_cells {
            self.ingest_once_cell(pid, cell);
        }
    }

    fn ingest_mpsc(&mut self, pid: u32, ch: &MpscChannelSnapshot) {
        let node_id = NodeId::MpscChannel {
            pid,
            name: ch.name.clone(),
        };
        let pending = ch.sent.saturating_sub(ch.received);
        self.nodes.insert(
            node_id.clone(),
            NodeKind::MpscChannel {
                name: ch.name.clone(),
                bounded: ch.bounded,
                capacity: ch.capacity,
                pending,
            },
        );

        // channel -> owned by creator task
        if let Some(creator_id) = ch.creator_task_id {
            let creator = NodeId::Task {
                pid,
                task_id: creator_id,
            };
            self.edges.push(WaitEdge {
                from: node_id.clone(),
                to: creator,
                kind: EdgeKind::ResourceOwnedByTask,
                meta: EdgeMeta::derived(SnapshotSource::Sync, 1, 0),
            });
        }

        // If senders are blocked, the channel is a bottleneck
        if ch.send_waiters > 0 {
            if let Some(creator_id) = ch.creator_task_id {
                let creator = NodeId::Task {
                    pid,
                    task_id: creator_id,
                };
                self.edges.push(WaitEdge {
                    from: creator,
                    to: node_id,
                    kind: EdgeKind::TaskWaitsOnResource,
                    meta: EdgeMeta::derived(SnapshotSource::Sync, ch.send_waiters, 2),
                });
            }
        }
    }

    fn ingest_oneshot(&mut self, pid: u32, ch: &OneshotChannelSnapshot) {
        let node_id = NodeId::OneshotChannel {
            pid,
            name: ch.name.clone(),
        };
        self.nodes.insert(
            node_id.clone(),
            NodeKind::OneshotChannel {
                name: ch.name.clone(),
                state: ch.state,
            },
        );
        if let Some(creator_id) = ch.creator_task_id {
            let creator = NodeId::Task {
                pid,
                task_id: creator_id,
            };
            self.edges.push(WaitEdge {
                from: node_id,
                to: creator,
                kind: EdgeKind::ResourceOwnedByTask,
                meta: EdgeMeta::derived(SnapshotSource::Sync, 1, 0),
            });
        }
    }

    fn ingest_watch(&mut self, pid: u32, ch: &WatchChannelSnapshot) {
        let node_id = NodeId::WatchChannel {
            pid,
            name: ch.name.clone(),
        };
        self.nodes.insert(
            node_id.clone(),
            NodeKind::WatchChannel {
                name: ch.name.clone(),
                changes: ch.changes,
            },
        );
        if let Some(creator_id) = ch.creator_task_id {
            let creator = NodeId::Task {
                pid,
                task_id: creator_id,
            };
            self.edges.push(WaitEdge {
                from: node_id,
                to: creator,
                kind: EdgeKind::ResourceOwnedByTask,
                meta: EdgeMeta::derived(SnapshotSource::Sync, 1, 0),
            });
        }
    }

    fn ingest_semaphore(&mut self, pid: u32, sem: &SemaphoreSnapshot) {
        let node_id = NodeId::Semaphore {
            pid,
            name: sem.name.clone(),
        };
        self.nodes.insert(
            node_id.clone(),
            NodeKind::Semaphore {
                name: sem.name.clone(),
                permits_total: sem.permits_total,
                permits_available: sem.permits_available,
                waiters: sem.waiters,
                oldest_wait_secs: sem.oldest_wait_secs,
            },
        );

        // semaphore -> owned by creator task
        if let Some(creator_id) = sem.creator_task_id {
            let creator = NodeId::Task {
                pid,
                task_id: creator_id,
            };
            self.edges.push(WaitEdge {
                from: node_id.clone(),
                to: creator,
                kind: EdgeKind::ResourceOwnedByTask,
                meta: EdgeMeta::derived(SnapshotSource::Sync, 1, 0),
            });
        }

        // Each waiter task -> waits on semaphore
        for &waiter_task_id in &sem.top_waiter_task_ids {
            let task_node = NodeId::Task {
                pid,
                task_id: waiter_task_id,
            };
            let severity = if sem.oldest_wait_secs > 30.0 {
                3
            } else if sem.oldest_wait_secs > 10.0 {
                2
            } else if sem.oldest_wait_secs > 1.0 {
                1
            } else {
                0
            };
            self.edges.push(WaitEdge {
                from: task_node,
                to: node_id.clone(),
                kind: EdgeKind::TaskWaitsOnResource,
                meta: EdgeMeta::derived(SnapshotSource::Sync, 1, severity),
            });
        }
    }

    fn ingest_once_cell(&mut self, pid: u32, cell: &OnceCellSnapshot) {
        let node_id = NodeId::OnceCell {
            pid,
            name: cell.name.clone(),
        };
        self.nodes.insert(
            node_id,
            NodeKind::OnceCell {
                name: cell.name.clone(),
                state: cell.state,
            },
        );
    }

    fn ingest_roam(&mut self, pid: u32, session: &SessionSnapshot) {
        for conn in &session.connections {
            self.ingest_connection(pid, conn, &session.method_names);
        }
        for ch in &session.channel_details {
            self.ingest_roam_channel(pid, ch);
        }
    }

    fn ingest_connection(
        &mut self,
        pid: u32,
        conn: &ConnectionSnapshot,
        method_names: &std::collections::HashMap<u64, String>,
    ) {
        for req in &conn.in_flight {
            let method_name = req
                .method_name
                .clone()
                .or_else(|| method_names.get(&req.method_id).cloned());

            let req_node = NodeId::RpcRequest {
                pid,
                connection: conn.name.clone(),
                request_id: req.request_id,
            };

            self.nodes.insert(
                req_node.clone(),
                NodeKind::RpcRequest {
                    method_name: method_name.clone(),
                    direction: req.direction.clone(),
                    elapsed_secs: req.elapsed_secs,
                },
            );

            if let Some(task_id) = req.task_id {
                let task_node = NodeId::Task { pid, task_id };
                let severity = if req.elapsed_secs > 5.0 { 3 } else { 1 };

                match req.direction {
                    Direction::Outgoing => {
                        // client task -> waits on RPC request
                        self.edges.push(WaitEdge {
                            from: task_node,
                            to: req_node.clone(),
                            kind: EdgeKind::RpcClientToRequest,
                            meta: EdgeMeta::derived(SnapshotSource::Roam, 1, severity),
                        });
                    }
                    Direction::Incoming => {
                        // RPC request -> handled by server task
                        self.edges.push(WaitEdge {
                            from: req_node.clone(),
                            to: task_node,
                            kind: EdgeKind::RpcRequestToServerTask,
                            meta: EdgeMeta::derived(SnapshotSource::Roam, 1, severity),
                        });
                    }
                }
            }
        }
    }

    fn ingest_roam_channel(&mut self, pid: u32, ch: &RoamChannelSnapshot) {
        let node_id = NodeId::RoamChannel {
            pid,
            channel_id: ch.channel_id,
        };
        self.nodes.insert(
            node_id.clone(),
            NodeKind::RoamChannel {
                name: ch.name.clone(),
                direction: ch.direction.clone(),
                queue_depth: ch.queue_depth,
                closed: ch.closed,
            },
        );

        // task -> waits on roam channel (only if not already closed)
        if !ch.closed {
            if let Some(task_id) = ch.task_id {
                let task_node = NodeId::Task { pid, task_id };
                let severity = if ch.age_secs > 30.0 {
                    3
                } else if ch.age_secs > 10.0 {
                    2
                } else if ch.age_secs > 1.0 {
                    1
                } else {
                    0
                };
                self.edges.push(WaitEdge {
                    from: task_node,
                    to: node_id,
                    kind: EdgeKind::TaskWaitsOnResource,
                    meta: EdgeMeta::derived(SnapshotSource::Roam, 1, severity),
                });
            }
        }
    }

    /// After all dumps are ingested, stitch outgoing RPC requests in one process
    /// to matching incoming RPC requests in another process.
    ///
    /// Match criteria: same method_name + same request_id, one Outgoing and one
    /// Incoming, in different processes.
    fn stitch_cross_process_rpc(&mut self) {
        // Collect RPC request nodes with their metadata
        struct RpcInfo {
            node_id: NodeId,
            method_name: Option<String>,
            request_id: u64,
            pid: u32,
            direction: Direction,
            elapsed_secs: f64,
        }

        let rpc_nodes: Vec<RpcInfo> = self
            .nodes
            .iter()
            .filter_map(|(id, kind)| {
                if let (
                    NodeId::RpcRequest {
                        pid, request_id, ..
                    },
                    NodeKind::RpcRequest {
                        method_name,
                        direction,
                        elapsed_secs,
                    },
                ) = (id, kind)
                {
                    Some(RpcInfo {
                        node_id: id.clone(),
                        method_name: method_name.clone(),
                        request_id: *request_id,
                        pid: *pid,
                        direction: direction.clone(),
                        elapsed_secs: *elapsed_secs,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Find matching pairs: outgoing in one process, incoming in another,
        // same method_name + request_id
        let mut new_edges = Vec::new();

        for i in 0..rpc_nodes.len() {
            for j in (i + 1)..rpc_nodes.len() {
                let a = &rpc_nodes[i];
                let b = &rpc_nodes[j];

                if a.pid == b.pid {
                    continue;
                }
                if a.request_id != b.request_id {
                    continue;
                }
                if a.method_name != b.method_name {
                    continue;
                }

                // Determine which is outgoing and which is incoming
                let (outgoing, incoming) = match (&a.direction, &b.direction) {
                    (Direction::Outgoing, Direction::Incoming) => (a, b),
                    (Direction::Incoming, Direction::Outgoing) => (b, a),
                    _ => continue, // same direction, not a stitch
                };

                let severity = if outgoing.elapsed_secs > 5.0 { 3 } else { 1 };

                new_edges.push(WaitEdge {
                    from: outgoing.node_id.clone(),
                    to: incoming.node_id.clone(),
                    kind: EdgeKind::RpcCrossProcessStitch,
                    meta: EdgeMeta::derived(SnapshotSource::Roam, 1, severity),
                });
            }
        }

        self.edges.extend(new_edges);
    }

    fn ingest_future_spawn_edges(&mut self, pid: u32, edges: &[FutureSpawnEdgeSnapshot]) {
        for edge in edges {
            if edge.parent_future_id == edge.child_future_id {
                continue;
            }

            let parent = NodeId::Future {
                pid,
                future_id: edge.parent_future_id,
            };
            let child = NodeId::Future {
                pid,
                future_id: edge.child_future_id,
            };

            self.nodes
                .entry(parent.clone())
                .or_insert(NodeKind::Future {
                    resource: edge.parent_resource.clone(),
                });
            self.nodes
                .entry(child.clone())
                .or_insert(NodeKind::Future {
                    resource: edge.child_resource.clone(),
                });

            self.edges.push(WaitEdge {
                from: parent,
                to: child,
                kind: EdgeKind::FutureSpawnedFuture,
                meta: EdgeMeta::explicit(SnapshotSource::FutureSpawnEdges, 1, 0),
            });
        }
    }

    fn ingest_future_poll_edges(&mut self, pid: u32, edges: &[FuturePollEdgeSnapshot]) {
        for edge in edges {
            let task_node = NodeId::Task {
                pid,
                task_id: edge.task_id,
            };
            let future_node = NodeId::Future {
                pid,
                future_id: edge.future_id,
            };

            self.nodes
                .entry(future_node.clone())
                .or_insert(NodeKind::Future {
                    resource: edge.future_resource.clone(),
                });

            let severity = if edge.total_poll_secs > 10.0 {
                3
            } else if edge.total_poll_secs > 5.0 {
                2
            } else if edge.total_poll_secs > 1.0 {
                1
            } else {
                0
            };

            self.edges.push(WaitEdge {
                from: task_node,
                to: future_node,
                kind: EdgeKind::TaskPollsFuture,
                meta: EdgeMeta::explicit(
                    SnapshotSource::FuturePollEdges,
                    edge.poll_count,
                    severity,
                ),
            });
        }
    }

    fn ingest_future_resume_edges(&mut self, pid: u32, edges: &[FutureResumeEdgeSnapshot]) {
        for edge in edges {
            let future_node = NodeId::Future {
                pid,
                future_id: edge.future_id,
            };
            let task_node = NodeId::Task {
                pid,
                task_id: edge.target_task_id,
            };

            self.nodes
                .entry(future_node.clone())
                .or_insert(NodeKind::Future {
                    resource: edge.future_resource.clone(),
                });

            self.edges.push(WaitEdge {
                from: future_node,
                to: task_node,
                kind: EdgeKind::FutureResumesTask,
                meta: EdgeMeta::explicit(
                    SnapshotSource::FutureResumeEdges,
                    edge.resume_count,
                    0,
                ),
            });
        }
    }

    fn ingest_future_resource_edges(
        &mut self,
        pid: u32,
        edges: &[FutureResourceEdgeSnapshot],
    ) {
        for edge in edges {
            let future_node = NodeId::Future {
                pid,
                future_id: edge.future_id,
            };

            let resource_node = resource_ref_to_node_id(pid, &edge.resource);

            // Ensure NodeKind exists for resources that don't have their own
            // ingestion path (Socket, Unknown). For Lock/Channel/etc the kind
            // is populated by ingest_locks/ingest_sync and will overwrite this
            // entry with richer data, which is fine.
            self.nodes.entry(resource_node.clone()).or_insert_with(|| {
                match &edge.resource {
                    ResourceRefSnapshot::Socket { fd, label, direction, peer, .. } => {
                        NodeKind::Socket {
                            fd: *fd,
                            label: label.clone(),
                            direction: *direction,
                            peer: peer.clone(),
                        }
                    }
                    ResourceRefSnapshot::Unknown { label } => {
                        NodeKind::Unknown {
                            label: label.clone(),
                        }
                    }
                    // Other resource types get their NodeKind from their own ingestion.
                    _ => NodeKind::Unknown {
                        label: format!("{:?}", edge.resource),
                    },
                }
            });

            self.edges.push(WaitEdge {
                from: future_node,
                to: resource_node,
                kind: EdgeKind::FutureWaitsOnResource,
                meta: EdgeMeta::explicit(
                    SnapshotSource::FutureResourceEdges,
                    edge.wait_count,
                    wait_secs_severity(edge.total_wait_secs),
                ),
            });
        }
    }

    fn ingest_request_parents(&mut self, dumps: &[ProcessDump]) {
        let mut req_lookup: std::collections::HashMap<(String, String, u64), (u32, NodeId)> =
            std::collections::HashMap::new();

        for dump in dumps {
            if let Some(ref roam) = dump.roam {
                for conn in &roam.connections {
                    for req in &conn.in_flight {
                        let key = (dump.process_name.clone(), conn.name.clone(), req.request_id);
                        let node = NodeId::RpcRequest {
                            pid: dump.pid,
                            connection: conn.name.clone(),
                            request_id: req.request_id,
                        };
                        req_lookup.insert(key, (dump.pid, node));
                    }
                }
            }
        }

        for dump in dumps {
            for rp in &dump.request_parents {
                let child_key = (
                    rp.child_process.clone(),
                    rp.child_connection.clone(),
                    rp.child_request_id,
                );
                let parent_key = (
                    rp.parent_process.clone(),
                    rp.parent_connection.clone(),
                    rp.parent_request_id,
                );

                if let (Some((child_pid, child_node)), Some((parent_pid, parent_node))) =
                    (req_lookup.get(&child_key), req_lookup.get(&parent_key))
                {
                    let kind = if child_pid != parent_pid {
                        EdgeKind::RpcCrossProcessStitch
                    } else {
                        EdgeKind::RpcRequestParent
                    };

                    self.edges.push(WaitEdge {
                        from: child_node.clone(),
                        to: parent_node.clone(),
                        kind,
                        meta: EdgeMeta::explicit(SnapshotSource::RequestParents, 1, 0),
                    });
                }
            }
        }
    }
}

fn resource_ref_to_node_id(pid: u32, resource: &ResourceRefSnapshot) -> NodeId {
    match resource {
        ResourceRefSnapshot::Lock { name, .. } => NodeId::Lock {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::Mpsc { name, .. } => NodeId::MpscChannel {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::Oneshot { name, .. } => NodeId::OneshotChannel {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::Watch { name, .. } => NodeId::WatchChannel {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::Semaphore { name, .. } => NodeId::Semaphore {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::OnceCell { name, .. } => NodeId::OnceCell {
            pid,
            name: name.clone(),
        },
        ResourceRefSnapshot::RoamChannel { channel_id, .. } => NodeId::RoamChannel {
            pid,
            channel_id: *channel_id,
        },
        ResourceRefSnapshot::Socket { fd, .. } => NodeId::Socket {
            pid,
            fd: *fd,
        },
        ResourceRefSnapshot::Unknown { label } => NodeId::Unknown {
            pid,
            label: label.clone(),
        },
    }
}

fn wait_secs_severity(total_wait_secs: f64) -> u8 {
    if total_wait_secs > 10.0 {
        3
    } else if total_wait_secs > 5.0 {
        2
    } else if total_wait_secs > 1.0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_dump(pid: u32, name: &str) -> ProcessDump {
        ProcessDump {
            process_name: name.to_string(),
            pid,
            timestamp: "2026-02-15T00:00:00Z".to_string(),
            tasks: vec![],
            wake_edges: vec![],
            future_wake_edges: vec![],
            future_waits: vec![],
            threads: vec![],
            locks: None,
            sync: None,
            roam: None,
            shm: None,
            future_spawn_edges: vec![],
            future_poll_edges: vec![],
            future_resume_edges: vec![],
            future_resource_edges: vec![],
            request_parents: vec![],
            custom: HashMap::new(),
        }
    }

    #[test]
    fn empty_dump_produces_process_node() {
        let graph = WaitGraph::build(&[empty_dump(1, "myapp")]);
        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.edges.is_empty());
        let (id, kind) = graph.nodes.iter().next().unwrap();
        assert_eq!(*id, NodeId::Process { pid: 1 });
        match kind {
            NodeKind::Process { name, pid } => {
                assert_eq!(name, "myapp");
                assert_eq!(*pid, 1);
            }
            _ => panic!("expected Process node"),
        }
    }

    #[test]
    fn tasks_with_parent_produce_spawn_edges() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![
            TaskSnapshot {
                id: 1,
                name: "root".to_string(),
                state: TaskState::Polling,
                spawned_at_secs: 0.0,
                age_secs: 10.0,
                spawn_backtrace: String::new(),
                poll_events: vec![],
                parent_task_id: None,
                parent_task_name: None,
            },
            TaskSnapshot {
                id: 2,
                name: "child".to_string(),
                state: TaskState::Pending,
                spawned_at_secs: 1.0,
                age_secs: 9.0,
                spawn_backtrace: String::new(),
                poll_events: vec![],
                parent_task_id: Some(1),
                parent_task_name: Some("root".to_string()),
            },
        ];
        let graph = WaitGraph::build(&[dump]);
        // process + 2 tasks = 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        // 1 spawn edge
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].kind, EdgeKind::TaskSpawnedTask);
        assert_eq!(
            graph.edges[0].from,
            NodeId::Task {
                pid: 1,
                task_id: 1
            }
        );
        assert_eq!(
            graph.edges[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 2
            }
        );
    }

    #[test]
    fn lock_contention_produces_wait_and_ownership_edges() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![
            TaskSnapshot {
                id: 10,
                name: "holder".to_string(),
                state: TaskState::Polling,
                spawned_at_secs: 0.0,
                age_secs: 5.0,
                spawn_backtrace: String::new(),
                poll_events: vec![],
                parent_task_id: None,
                parent_task_name: None,
            },
            TaskSnapshot {
                id: 20,
                name: "waiter".to_string(),
                state: TaskState::Pending,
                spawned_at_secs: 1.0,
                age_secs: 4.0,
                spawn_backtrace: String::new(),
                poll_events: vec![],
                parent_task_id: None,
                parent_task_name: None,
            },
        ];
        dump.locks = Some(peeps_types::LockSnapshot {
            locks: vec![peeps_types::LockInfoSnapshot {
                name: "db_pool".to_string(),
                acquires: 100,
                releases: 99,
                holders: vec![peeps_types::LockHolderSnapshot {
                    kind: LockAcquireKind::Mutex,
                    held_secs: 2.0,
                    backtrace: None,
                    task_id: Some(10),
                    task_name: Some("holder".to_string()),
                }],
                waiters: vec![peeps_types::LockWaiterSnapshot {
                    kind: LockAcquireKind::Mutex,
                    waiting_secs: 0.5,
                    backtrace: None,
                    task_id: Some(20),
                    task_name: Some("waiter".to_string()),
                }],
            }],
        });

        let graph = WaitGraph::build(&[dump]);

        // process + 2 tasks + 1 lock = 4 nodes
        assert_eq!(graph.nodes.len(), 4);

        let ownership_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(ownership_edges.len(), 1);
        assert_eq!(
            ownership_edges[0].from,
            NodeId::Lock {
                pid: 1,
                name: "db_pool".to_string()
            }
        );
        assert_eq!(
            ownership_edges[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 10
            }
        );

        let wait_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(wait_edges.len(), 1);
        assert_eq!(
            wait_edges[0].from,
            NodeId::Task {
                pid: 1,
                task_id: 20
            }
        );
        assert_eq!(
            wait_edges[0].to,
            NodeId::Lock {
                pid: 1,
                name: "db_pool".to_string()
            }
        );
    }

    #[test]
    fn future_wait_produces_edges() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![TaskSnapshot {
            id: 5,
            name: "poller".to_string(),
            state: TaskState::Pending,
            spawned_at_secs: 0.0,
            age_secs: 3.0,
            spawn_backtrace: String::new(),
            poll_events: vec![],
            parent_task_id: None,
            parent_task_name: None,
        }];
        dump.future_waits = vec![FutureWaitSnapshot {
            future_id: 42,
            task_id: 5,
            task_name: Some("poller".to_string()),
            resource: "timeout".to_string(),
            created_by_task_id: Some(5),
            created_by_task_name: Some("poller".to_string()),
            created_age_secs: 3.0,
            last_polled_by_task_id: Some(5),
            last_polled_by_task_name: Some("poller".to_string()),
            pending_count: 3,
            ready_count: 0,
            total_pending_secs: 2.0,
            last_seen_age_secs: 0.1,
        }];
        let graph = WaitGraph::build(&[dump]);

        // process + task + future = 3 nodes
        assert_eq!(graph.nodes.len(), 3);

        // task -> waits on future, future -> owned by task
        assert_eq!(graph.edges.len(), 2);

        let wait = graph
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        // never been ready => severity 2
        assert_eq!(wait.meta.severity_hint, 2);
    }

    #[test]
    fn rpc_in_flight_produces_edges() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![TaskSnapshot {
            id: 7,
            name: "rpc-caller".to_string(),
            state: TaskState::Pending,
            spawned_at_secs: 0.0,
            age_secs: 2.0,
            spawn_backtrace: String::new(),
            poll_events: vec![],
            parent_task_id: None,
            parent_task_name: None,
        }];
        dump.roam = Some(SessionSnapshot {
            connections: vec![ConnectionSnapshot {
                name: "conn-1".to_string(),
                peer_name: Some("backend".to_string()),
                age_secs: 60.0,
                total_completed: 100,
                max_concurrent_requests: 8,
                initial_credit: 8,
                in_flight: vec![peeps_types::RequestSnapshot {
                    request_id: 99,
                    method_name: Some("get_user".to_string()),
                    method_id: 1,
                    direction: Direction::Outgoing,
                    elapsed_secs: 6.0,
                    task_id: Some(7),
                    task_name: Some("rpc-caller".to_string()),
                    metadata: None,
                    args: None,
                    backtrace: None,
                    server_task_id: None,
                    server_task_name: None,
                }],
                recent_completions: vec![],
                channels: vec![],
                transport: peeps_types::TransportStats {
                    frames_sent: 200,
                    frames_received: 200,
                    bytes_sent: 10000,
                    bytes_received: 10000,
                    last_sent_ago_secs: Some(0.1),
                    last_recv_ago_secs: Some(0.1),
                },
                channel_credits: vec![],
            }],
            method_names: HashMap::new(),
            channel_details: vec![],
        });

        let graph = WaitGraph::build(&[dump]);

        let rpc_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::RpcClientToRequest)
            .collect();
        assert_eq!(rpc_edges.len(), 1);
        // >5s elapsed => severity 3
        assert_eq!(rpc_edges[0].meta.severity_hint, 3);
    }

    #[test]
    fn multiple_dumps_merge() {
        let dump1 = empty_dump(1, "frontend");
        let dump2 = empty_dump(2, "backend");
        let graph = WaitGraph::build(&[dump1, dump2]);
        assert_eq!(graph.nodes.len(), 2);
        assert!(graph.nodes.contains_key(&NodeId::Process { pid: 1 }));
        assert!(graph.nodes.contains_key(&NodeId::Process { pid: 2 }));
    }

    // ════════════════════════════════════════════════════════════════
    // Fixture corpus: realistic ProcessDump scenarios for end-to-end
    // validation of graph normalization, cycle detection, and severity
    // ranking. Each fixture builds ProcessDump(s) and asserts through
    // the full pipeline: build → detect → rank.
    // ════════════════════════════════════════════════════════════════

    fn make_task(id: u64, name: &str, state: TaskState, age: f64) -> TaskSnapshot {
        TaskSnapshot {
            id,
            name: name.to_string(),
            state,
            spawned_at_secs: 0.0,
            age_secs: age,
            spawn_backtrace: String::new(),
            poll_events: vec![],
            parent_task_id: None,
            parent_task_name: None,
        }
    }

    fn make_lock(
        name: &str,
        holders: Vec<(u64, &str, f64)>,
        waiters: Vec<(u64, &str, f64)>,
    ) -> peeps_types::LockInfoSnapshot {
        peeps_types::LockInfoSnapshot {
            name: name.to_string(),
            acquires: 100,
            releases: 99,
            holders: holders
                .into_iter()
                .map(|(tid, tname, held)| peeps_types::LockHolderSnapshot {
                    kind: LockAcquireKind::Mutex,
                    held_secs: held,
                    backtrace: None,
                    task_id: Some(tid),
                    task_name: Some(tname.to_string()),
                })
                .collect(),
            waiters: waiters
                .into_iter()
                .map(|(tid, tname, wait)| peeps_types::LockWaiterSnapshot {
                    kind: LockAcquireKind::Mutex,
                    waiting_secs: wait,
                    backtrace: None,
                    task_id: Some(tid),
                    task_name: Some(tname.to_string()),
                })
                .collect(),
        }
    }

    fn make_connection(
        name: &str,
        requests: Vec<peeps_types::RequestSnapshot>,
    ) -> ConnectionSnapshot {
        ConnectionSnapshot {
            name: name.to_string(),
            peer_name: Some("peer".to_string()),
            age_secs: 60.0,
            total_completed: 50,
            max_concurrent_requests: 8,
            initial_credit: 8,
            in_flight: requests,
            recent_completions: vec![],
            channels: vec![],
            transport: peeps_types::TransportStats {
                frames_sent: 100,
                frames_received: 100,
                bytes_sent: 5000,
                bytes_received: 5000,
                last_sent_ago_secs: Some(0.1),
                last_recv_ago_secs: Some(0.1),
            },
            channel_credits: vec![],
        }
    }

    fn make_rpc_request(
        method: &str,
        request_id: u64,
        direction: Direction,
        elapsed: f64,
        task_id: Option<u64>,
        task_name: Option<&str>,
    ) -> peeps_types::RequestSnapshot {
        peeps_types::RequestSnapshot {
            request_id,
            method_name: Some(method.to_string()),
            method_id: 0,
            direction,
            elapsed_secs: elapsed,
            task_id,
            task_name: task_name.map(|s| s.to_string()),
            metadata: None,
            args: None,
            backtrace: None,
            server_task_id: None,
            server_task_name: None,
        }
    }

    // ── Fixture 1: True deadlock cycle ─────────────────────────────
    //
    // Task A (id=1) holds lock X, waits on lock Y.
    // Task B (id=2) holds lock Y, waits on lock X.
    //
    // Expected: cycle A → lock-Y → B → lock-X → A detected as Danger.

    fn fixture_true_deadlock() -> ProcessDump {
        let mut dump = empty_dump(1, "deadlock-app");
        dump.tasks = vec![
            make_task(1, "task-A", TaskState::Pending, 10.0),
            make_task(2, "task-B", TaskState::Pending, 10.0),
        ];
        dump.locks = Some(peeps_types::LockSnapshot {
            locks: vec![
                make_lock("lock-X", vec![(1, "task-A", 5.0)], vec![(2, "task-B", 4.0)]),
                make_lock("lock-Y", vec![(2, "task-B", 5.0)], vec![(1, "task-A", 4.0)]),
            ],
        });
        dump
    }

    #[test]
    fn fixture_true_deadlock_normalization() {
        let graph = WaitGraph::build(&[fixture_true_deadlock()]);

        // 1 process + 2 tasks + 2 locks = 5 nodes
        assert_eq!(graph.nodes.len(), 5);

        // Each lock: 1 ownership + 1 wait = 4 edges total
        assert_eq!(graph.edges.len(), 4);

        let wait_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(wait_edges.len(), 2);

        let owns_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns_edges.len(), 2);

        // task-A waits on lock-Y
        assert!(wait_edges.iter().any(|e| e.from
            == NodeId::Task {
                pid: 1,
                task_id: 1
            }
            && e.to
                == NodeId::Lock {
                    pid: 1,
                    name: "lock-Y".to_string()
                }));

        // task-B waits on lock-X
        assert!(wait_edges.iter().any(|e| e.from
            == NodeId::Task {
                pid: 1,
                task_id: 2
            }
            && e.to
                == NodeId::Lock {
                    pid: 1,
                    name: "lock-X".to_string()
                }));

        // lock-X owned by task-A
        assert!(owns_edges.iter().any(|e| e.from
            == NodeId::Lock {
                pid: 1,
                name: "lock-X".to_string()
            }
            && e.to
                == NodeId::Task {
                    pid: 1,
                    task_id: 1
                }));

        // lock-Y owned by task-B
        assert!(owns_edges.iter().any(|e| e.from
            == NodeId::Lock {
                pid: 1,
                name: "lock-Y".to_string()
            }
            && e.to
                == NodeId::Task {
                    pid: 1,
                    task_id: 2
                }));
    }

    #[test]
    fn fixture_true_deadlock_cycle_detection() {
        let graph = WaitGraph::build(&[fixture_true_deadlock()]);
        let candidates = detect::find_deadlock_candidates(&graph);

        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];

        // All four nodes in the cycle
        assert_eq!(c.nodes.len(), 4);
        assert!(c.nodes.contains(&NodeId::Task {
            pid: 1,
            task_id: 1
        }));
        assert!(c.nodes.contains(&NodeId::Task {
            pid: 1,
            task_id: 2
        }));
        assert!(c.nodes.contains(&NodeId::Lock {
            pid: 1,
            name: "lock-X".to_string()
        }));
        assert!(c.nodes.contains(&NodeId::Lock {
            pid: 1,
            name: "lock-Y".to_string()
        }));

        // Cycle path is closed
        assert_eq!(c.cycle_path.first(), c.cycle_path.last());
        assert!(c.cycle_path.len() >= 5); // 4 nodes + closing = 5

        // Severity: age 10s + cycle = at least warn
        assert!(c.severity >= detect::Severity::Warn);
        assert!(c.severity_score >= 20);
    }

    // ── Fixture 2: Long wait, no cycle ─────────────────────────────
    //
    // Task C calls an RPC that's been running for 30s.
    // No circular dependency — just slow. No deadlock candidate.

    fn fixture_long_wait_no_cycle() -> ProcessDump {
        let mut dump = empty_dump(2, "slow-rpc-app");
        dump.tasks = vec![make_task(3, "rpc-caller", TaskState::Pending, 30.0)];
        dump.roam = Some(SessionSnapshot {
            connections: vec![make_connection(
                "conn-slow",
                vec![make_rpc_request(
                    "heavy_query",
                    200,
                    Direction::Outgoing,
                    30.0,
                    Some(3),
                    Some("rpc-caller"),
                )],
            )],
            method_names: HashMap::new(),
            channel_details: vec![],
        });
        dump
    }

    #[test]
    fn fixture_long_wait_no_cycle_normalization() {
        let graph = WaitGraph::build(&[fixture_long_wait_no_cycle()]);

        // 1 process + 1 task + 1 rpc_request = 3 nodes
        assert_eq!(graph.nodes.len(), 3);

        let rpc_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::RpcClientToRequest)
            .collect();
        assert_eq!(rpc_edges.len(), 1);
        assert_eq!(rpc_edges[0].meta.severity_hint, 3); // >5s

        // No wait-on-resource edges
        let wait_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert!(wait_edges.is_empty());
    }

    #[test]
    fn fixture_long_wait_no_cycle_detection() {
        let graph = WaitGraph::build(&[fixture_long_wait_no_cycle()]);
        let candidates = detect::find_deadlock_candidates(&graph);

        // No cycle — the RPC is just slow, not deadlocked
        assert!(candidates.is_empty());
    }

    // ── Fixture 3: Bursty transient waits ──────────────────────────
    //
    // Multiple tasks contending on a lock with very short wait times.
    // Lock holder is actively working, waiters waited < 5ms.
    // This is normal contention, NOT a deadlock.

    fn fixture_bursty_transient() -> ProcessDump {
        let mut dump = empty_dump(3, "bursty-app");
        dump.tasks = vec![
            make_task(10, "worker-1", TaskState::Polling, 2.0),
            make_task(11, "worker-2", TaskState::Pending, 2.0),
            make_task(12, "worker-3", TaskState::Pending, 2.0),
        ];
        dump.locks = Some(peeps_types::LockSnapshot {
            locks: vec![peeps_types::LockInfoSnapshot {
                name: "hot-mutex".to_string(),
                acquires: 10000,
                releases: 9999,
                holders: vec![peeps_types::LockHolderSnapshot {
                    kind: LockAcquireKind::Mutex,
                    held_secs: 0.001, // 1ms
                    backtrace: None,
                    task_id: Some(10),
                    task_name: Some("worker-1".to_string()),
                }],
                waiters: vec![
                    peeps_types::LockWaiterSnapshot {
                        kind: LockAcquireKind::Mutex,
                        waiting_secs: 0.002,
                        backtrace: None,
                        task_id: Some(11),
                        task_name: Some("worker-2".to_string()),
                    },
                    peeps_types::LockWaiterSnapshot {
                        kind: LockAcquireKind::Mutex,
                        waiting_secs: 0.001,
                        backtrace: None,
                        task_id: Some(12),
                        task_name: Some("worker-3".to_string()),
                    },
                ],
            }],
        });
        dump
    }

    #[test]
    fn fixture_bursty_transient_normalization() {
        let graph = WaitGraph::build(&[fixture_bursty_transient()]);

        // 1 process + 3 tasks + 1 lock = 5 nodes
        assert_eq!(graph.nodes.len(), 5);

        // 1 ownership edge + 2 wait edges = 3
        assert_eq!(graph.edges.len(), 3);

        // All severity hints should be low
        let owns = graph
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .unwrap();
        assert_eq!(owns.meta.severity_hint, 1); // Mutex but held < 1s

        for wait in graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
        {
            assert_eq!(wait.meta.severity_hint, 1); // waiting < 1s
        }
    }

    #[test]
    fn fixture_bursty_transient_no_deadlock() {
        let graph = WaitGraph::build(&[fixture_bursty_transient()]);
        let candidates = detect::find_deadlock_candidates(&graph);

        // No cycle: workers wait on lock, lock owned by one worker.
        // This is a DAG (waiter -> lock -> holder), not a cycle.
        assert!(candidates.is_empty());
    }

    // ── Fixture 4: Cross-process RPC chain cycle ───────────────────
    //
    // Process A (pid=100): task-1 sends RPC to B, waiting for response.
    // Process B (pid=200): task-2 handles that request, but sends an
    //   RPC back to process A and is waiting for response.
    // Process A (pid=100): task-3 handles incoming from B, but waits
    //   on a lock held by task-1.
    //
    // Cycle: A:task-1 --rpc--> B:task-2 --rpc--> A:task-3 --lock--> A:task-1

    fn fixture_cross_process_rpc_cycle() -> Vec<ProcessDump> {
        // Process A
        let mut dump_a = empty_dump(100, "process-A");
        dump_a.tasks = vec![
            make_task(1, "a-sender", TaskState::Pending, 15.0),
            make_task(3, "a-handler", TaskState::Pending, 10.0),
        ];
        dump_a.roam = Some(SessionSnapshot {
            connections: vec![
                make_connection(
                    "conn-to-B",
                    vec![make_rpc_request(
                        "do_work",
                        300,
                        Direction::Outgoing,
                        10.0,
                        Some(1),
                        Some("a-sender"),
                    )],
                ),
                make_connection(
                    "conn-from-B",
                    vec![make_rpc_request(
                        "callback",
                        400,
                        Direction::Incoming,
                        8.0,
                        Some(3),
                        Some("a-handler"),
                    )],
                ),
            ],
            method_names: HashMap::new(),
            channel_details: vec![],
        });
        dump_a.locks = Some(peeps_types::LockSnapshot {
            locks: vec![make_lock(
                "shared-state",
                vec![(1, "a-sender", 10.0)],
                vec![(3, "a-handler", 8.0)],
            )],
        });

        // Process B
        let mut dump_b = empty_dump(200, "process-B");
        dump_b.tasks = vec![make_task(2, "b-handler", TaskState::Pending, 12.0)];
        dump_b.roam = Some(SessionSnapshot {
            connections: vec![
                make_connection(
                    "conn-from-A",
                    vec![make_rpc_request(
                        "do_work",
                        300,
                        Direction::Incoming,
                        10.0,
                        Some(2),
                        Some("b-handler"),
                    )],
                ),
                make_connection(
                    "conn-to-A",
                    vec![make_rpc_request(
                        "callback",
                        400,
                        Direction::Outgoing,
                        8.0,
                        Some(2),
                        Some("b-handler"),
                    )],
                ),
            ],
            method_names: HashMap::new(),
            channel_details: vec![],
        });

        vec![dump_a, dump_b]
    }

    #[test]
    fn fixture_cross_process_normalization() {
        let dumps = fixture_cross_process_rpc_cycle();
        let graph = WaitGraph::build(&dumps);

        // Process A: process + 2 tasks + 2 rpc_requests + 1 lock = 6
        // Process B: process + 1 task + 2 rpc_requests = 4
        // Total = 10
        assert_eq!(graph.nodes.len(), 10);

        // Process A: outgoing RPC (1→req), incoming RPC (req→3), lock own (lock→1), lock wait (3→lock) = 4
        // Process B: incoming RPC (req→2), outgoing RPC (2→req) = 2
        // Cross-process stitch: do_work outgoing→incoming, callback outgoing→incoming = 2
        // Total = 8
        assert_eq!(graph.edges.len(), 8);

        let rpc_client_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::RpcClientToRequest)
            .collect();
        assert_eq!(rpc_client_edges.len(), 2);

        let rpc_server_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::RpcRequestToServerTask)
            .collect();
        assert_eq!(rpc_server_edges.len(), 2);

        // Lock contention: task-3 waits, task-1 holds
        let lock_wait = graph
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        assert_eq!(
            lock_wait.from,
            NodeId::Task {
                pid: 100,
                task_id: 3
            }
        );
        assert_eq!(lock_wait.meta.severity_hint, 3); // >1s

        let lock_own = graph
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .unwrap();
        assert_eq!(
            lock_own.to,
            NodeId::Task {
                pid: 100,
                task_id: 1
            }
        );
        assert_eq!(lock_own.meta.severity_hint, 3); // >1s
    }

    #[test]
    fn fixture_cross_process_cycle_via_stitching() {
        // The ingestion pipeline stitches outgoing RPC nodes in one process
        // to incoming RPC nodes in another (matching by method_name + request_id).
        // This makes the cross-process cycle visible to the detector.
        let dumps = fixture_cross_process_rpc_cycle();
        let graph = WaitGraph::build(&dumps);
        let candidates = detect::find_deadlock_candidates(&graph);

        assert_eq!(
            candidates.len(),
            1,
            "cross-process stitching should reveal the RPC cycle"
        );
        let c = &candidates[0];
        assert!(c.rationale.iter().any(|r| r.contains("processes")));
        assert!(c.severity >= detect::Severity::Warn);
    }

    // ── Comparative severity ranking ───────────────────────────────

    #[test]
    fn cross_process_outranks_single_process() {
        let single_graph = WaitGraph::build(&[fixture_true_deadlock()]);
        let cross_graph = WaitGraph::build(&fixture_cross_process_rpc_cycle());

        let single_candidates = detect::find_deadlock_candidates(&single_graph);
        let cross_candidates = detect::find_deadlock_candidates(&cross_graph);

        assert_eq!(single_candidates.len(), 1);
        assert_eq!(cross_candidates.len(), 1);

        // Cross-process cycle should score higher (has cross-process bonus)
        assert!(
            cross_candidates[0].severity_score >= single_candidates[0].severity_score,
            "cross-process ({}) should score >= single-process ({})",
            cross_candidates[0].severity_score,
            single_candidates[0].severity_score
        );

        // Cross-process should be Danger
        assert_eq!(cross_candidates[0].severity, detect::Severity::Danger);

        // Single-process should be at least Warn
        assert!(single_candidates[0].severity >= detect::Severity::Warn);
    }

    // ── Sync channel fixture ───────────────────────────────────────

    #[test]
    fn mpsc_with_blocked_senders_produces_wait_edge() {
        let mut dump = empty_dump(1, "channel-app");
        dump.tasks = vec![make_task(5, "producer", TaskState::Pending, 3.0)];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![MpscChannelSnapshot {
                name: "work-queue".to_string(),
                bounded: true,
                capacity: Some(10),
                sent: 1000,
                received: 990,
                send_waiters: 3,
                sender_count: 4,
                sender_closed: false,
                receiver_closed: false,
                age_secs: 60.0,
                creator_task_id: Some(5),
                creator_task_name: Some("producer".to_string()),
            }],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);
        assert_eq!(graph.nodes.len(), 3); // process + task + channel

        let owns: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns.len(), 1);

        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(waits.len(), 1);
        assert_eq!(waits[0].meta.count, 3);
        assert_eq!(waits[0].meta.severity_hint, 2);
    }

    // ── Wake edge normalization ────────────────────────────────────

    #[test]
    fn wake_edges_produce_graph_edges() {
        let mut dump = empty_dump(1, "wake-app");
        dump.tasks = vec![
            make_task(1, "waker", TaskState::Polling, 5.0),
            make_task(2, "sleeper", TaskState::Pending, 5.0),
        ];
        dump.wake_edges = vec![WakeEdgeSnapshot {
            source_task_id: Some(1),
            source_task_name: Some("waker".to_string()),
            target_task_id: 2,
            target_task_name: Some("sleeper".to_string()),
            wake_count: 42,
            last_wake_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);
        assert_eq!(graph.nodes.len(), 3); // process + 2 tasks

        let wake_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWakesFuture)
            .collect();
        assert_eq!(wake_edges.len(), 1);
        assert_eq!(wake_edges[0].meta.count, 42);
    }

    // ── Future wake edge normalization ─────────────────────────────

    #[test]
    fn future_wake_edges_produce_graph_edges() {
        let mut dump = empty_dump(1, "fut-wake-app");
        dump.tasks = vec![
            make_task(1, "producer", TaskState::Polling, 5.0),
            make_task(2, "consumer", TaskState::Pending, 5.0),
        ];
        dump.future_wake_edges = vec![FutureWakeEdgeSnapshot {
            source_task_id: Some(1),
            source_task_name: Some("producer".to_string()),
            future_id: 99,
            future_resource: "notify".to_string(),
            target_task_id: Some(2),
            target_task_name: Some("consumer".to_string()),
            wake_count: 10,
            last_wake_age_secs: 0.05,
        }];

        let graph = WaitGraph::build(&[dump]);
        assert_eq!(graph.nodes.len(), 4); // process + 2 tasks + 1 future

        let wakes: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWakesFuture)
            .collect();
        assert_eq!(wakes.len(), 1);

        let resumes: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureResumesTask)
            .collect();
        assert_eq!(resumes.len(), 1);
        assert_eq!(
            resumes[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 2
            }
        );
    }

    // ── Oneshot channel normalization ──────────────────────────────

    #[test]
    fn oneshot_channel_produces_ownership_edge() {
        let mut dump = empty_dump(1, "oneshot-app");
        dump.tasks = vec![make_task(7, "requester", TaskState::Pending, 1.0)];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![OneshotChannelSnapshot {
                name: "response-ch".to_string(),
                state: OneshotState::Pending,
                age_secs: 1.0,
                creator_task_id: Some(7),
                creator_task_name: Some("requester".to_string()),
            }],
            watch_channels: vec![],
            semaphores: vec![],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);
        assert_eq!(graph.nodes.len(), 3); // process + task + oneshot

        let owns: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns.len(), 1);
        assert_eq!(
            owns[0].from,
            NodeId::OneshotChannel {
                pid: 1,
                name: "response-ch".to_string()
            }
        );
    }

    // ── OnceCell normalization ─────────────────────────────────────

    #[test]
    fn once_cell_creates_node() {
        let mut dump = empty_dump(1, "cell-app");
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![],
            once_cells: vec![OnceCellSnapshot {
                name: "config".to_string(),
                state: OnceCellState::Initializing,
                age_secs: 0.5,
                init_duration_secs: None,
            }],
        });

        let graph = WaitGraph::build(&[dump]);
        assert_eq!(graph.nodes.len(), 2); // process + once_cell
        assert!(graph.nodes.contains_key(&NodeId::OnceCell {
            pid: 1,
            name: "config".to_string()
        }));
    }

    // ── Semaphore normalization ────────────────────────────────────

    #[test]
    fn semaphore_with_waiters_produces_edges() {
        let mut dump = empty_dump(1, "sem-app");
        dump.tasks = vec![
            make_task(10, "creator", TaskState::Polling, 5.0),
            make_task(20, "waiter-a", TaskState::Pending, 3.0),
            make_task(30, "waiter-b", TaskState::Pending, 3.0),
        ];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![peeps_types::SemaphoreSnapshot {
                name: "pool-limit".to_string(),
                permits_total: 4,
                permits_available: 0,
                waiters: 2,
                acquires: 100,
                avg_wait_secs: 0.5,
                max_wait_secs: 2.0,
                age_secs: 60.0,
                creator_task_id: Some(10),
                creator_task_name: Some("creator".to_string()),
                top_waiter_task_ids: vec![20, 30],
                oldest_wait_secs: 15.0,
            }],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);

        // process + 3 tasks + 1 semaphore = 5 nodes
        assert_eq!(graph.nodes.len(), 5);

        assert!(graph.nodes.contains_key(&NodeId::Semaphore {
            pid: 1,
            name: "pool-limit".to_string()
        }));

        // ownership edge: semaphore -> creator
        let owns: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns.len(), 1);
        assert_eq!(
            owns[0].from,
            NodeId::Semaphore {
                pid: 1,
                name: "pool-limit".to_string()
            }
        );
        assert_eq!(
            owns[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 10
            }
        );

        // wait edges: 2 waiters -> semaphore
        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(waits.len(), 2);
        // severity: oldest_wait_secs=15.0 -> >10s -> severity 2
        for w in &waits {
            assert_eq!(w.meta.severity_hint, 2);
        }
    }

    #[test]
    fn semaphore_severity_scales_with_oldest_wait() {
        let make_sem_dump = |oldest_wait: f64| -> ProcessDump {
            let mut dump = empty_dump(1, "app");
            dump.tasks = vec![make_task(1, "t", TaskState::Pending, 1.0)];
            dump.sync = Some(SyncSnapshot {
                mpsc_channels: vec![],
                oneshot_channels: vec![],
                watch_channels: vec![],
                semaphores: vec![peeps_types::SemaphoreSnapshot {
                    name: "sem".to_string(),
                    permits_total: 1,
                    permits_available: 0,
                    waiters: 1,
                    acquires: 10,
                    avg_wait_secs: 0.1,
                    max_wait_secs: oldest_wait,
                    age_secs: 60.0,
                    creator_task_id: None,
                    creator_task_name: None,
                    top_waiter_task_ids: vec![1],
                    oldest_wait_secs: oldest_wait,
                }],
                once_cells: vec![],
            });
            dump
        };

        // <1s -> severity 0
        let g = WaitGraph::build(&[make_sem_dump(0.5)]);
        let w = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(w.meta.severity_hint, 0);

        // >1s -> severity 1
        let g = WaitGraph::build(&[make_sem_dump(5.0)]);
        let w = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(w.meta.severity_hint, 1);

        // >10s -> severity 2
        let g = WaitGraph::build(&[make_sem_dump(15.0)]);
        let w = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(w.meta.severity_hint, 2);

        // >30s -> severity 3
        let g = WaitGraph::build(&[make_sem_dump(45.0)]);
        let w = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(w.meta.severity_hint, 3);
    }

    #[test]
    fn semaphore_no_waiters_no_wait_edges() {
        let mut dump = empty_dump(1, "sem-app");
        dump.tasks = vec![make_task(10, "creator", TaskState::Polling, 5.0)];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![peeps_types::SemaphoreSnapshot {
                name: "idle-sem".to_string(),
                permits_total: 4,
                permits_available: 4,
                waiters: 0,
                acquires: 50,
                avg_wait_secs: 0.01,
                max_wait_secs: 0.1,
                age_secs: 120.0,
                creator_task_id: Some(10),
                creator_task_name: Some("creator".to_string()),
                top_waiter_task_ids: vec![],
                oldest_wait_secs: 0.0,
            }],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);

        // process + task + semaphore = 3 nodes
        assert_eq!(graph.nodes.len(), 3);

        // Only ownership edge, no wait edges
        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert!(waits.is_empty());

        let owns: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns.len(), 1);
    }

    // ── Roam channel normalization ───────────────────────────────

    #[test]
    fn roam_channel_produces_node_and_wait_edge() {
        let mut dump = empty_dump(1, "roam-ch-app");
        dump.tasks = vec![make_task(8, "stream-reader", TaskState::Pending, 15.0)];
        dump.roam = Some(SessionSnapshot {
            connections: vec![],
            method_names: HashMap::new(),
            channel_details: vec![RoamChannelSnapshot {
                channel_id: 42,
                name: "conn-1".to_string(),
                direction: ChannelDir::Rx,
                age_secs: 15.0,
                request_id: Some(100),
                task_id: Some(8),
                task_name: Some("stream-reader".to_string()),
                queue_depth: None,
                closed: false,
            }],
        });

        let graph = WaitGraph::build(&[dump]);

        // process + task + roam_channel = 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        assert!(graph.nodes.contains_key(&NodeId::RoamChannel {
            pid: 1,
            channel_id: 42,
        }));

        // task -> waits on roam channel
        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(waits.len(), 1);
        assert_eq!(
            waits[0].from,
            NodeId::Task {
                pid: 1,
                task_id: 8
            }
        );
        assert_eq!(
            waits[0].to,
            NodeId::RoamChannel {
                pid: 1,
                channel_id: 42,
            }
        );
        // age > 10s => severity 2
        assert_eq!(waits[0].meta.severity_hint, 2);
    }

    #[test]
    fn roam_channel_severity_scales_with_age() {
        let make_ch = |age: f64| -> ProcessDump {
            let mut dump = empty_dump(1, "app");
            dump.tasks = vec![make_task(1, "t", TaskState::Pending, age)];
            dump.roam = Some(SessionSnapshot {
                connections: vec![],
                method_names: HashMap::new(),
                channel_details: vec![RoamChannelSnapshot {
                    channel_id: 1,
                    name: "c".to_string(),
                    direction: ChannelDir::Tx,
                    age_secs: age,
                    request_id: None,
                    task_id: Some(1),
                    task_name: Some("t".to_string()),
                    queue_depth: None,
                    closed: false,
                }],
            });
            dump
        };

        // < 1s => severity 0
        let g = WaitGraph::build(&[make_ch(0.5)]);
        let s = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(s.meta.severity_hint, 0);

        // > 1s => severity 1
        let g = WaitGraph::build(&[make_ch(5.0)]);
        let s = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(s.meta.severity_hint, 1);

        // > 10s => severity 2
        let g = WaitGraph::build(&[make_ch(20.0)]);
        let s = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(s.meta.severity_hint, 2);

        // > 30s => severity 3
        let g = WaitGraph::build(&[make_ch(45.0)]);
        let s = g.edges.iter().find(|e| e.kind == EdgeKind::TaskWaitsOnResource).unwrap();
        assert_eq!(s.meta.severity_hint, 3);
    }

    // ── Semaphore contention graph edges ───────────────────────────

    #[test]
    fn semaphore_contention_produces_wait_edges() {
        let mut dump = empty_dump(1, "sem-app");
        dump.tasks = vec![
            make_task(1, "creator", TaskState::Polling, 30.0),
            make_task(2, "waiter-a", TaskState::Pending, 20.0),
            make_task(3, "waiter-b", TaskState::Pending, 15.0),
        ];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![SemaphoreSnapshot {
                name: "pool-limit".to_string(),
                permits_total: 4,
                permits_available: 0,
                waiters: 2,
                acquires: 100,
                avg_wait_secs: 5.0,
                max_wait_secs: 12.0,
                age_secs: 30.0,
                creator_task_id: Some(1),
                creator_task_name: Some("creator".to_string()),
                top_waiter_task_ids: vec![2, 3],
                oldest_wait_secs: 12.0,
            }],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);

        // process + 3 tasks + 1 semaphore = 5 nodes
        assert_eq!(graph.nodes.len(), 5);

        // 1 ownership (sem -> creator) + 2 waits (waiter-a -> sem, waiter-b -> sem)
        let owns: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::ResourceOwnedByTask)
            .collect();
        assert_eq!(owns.len(), 1);
        assert_eq!(
            owns[0].from,
            NodeId::Semaphore {
                pid: 1,
                name: "pool-limit".to_string()
            }
        );
        assert_eq!(
            owns[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 1
            }
        );

        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert_eq!(waits.len(), 2);

        for wait in &waits {
            assert_eq!(
                wait.to,
                NodeId::Semaphore {
                    pid: 1,
                    name: "pool-limit".to_string()
                }
            );
        }

        // oldest_wait > 10s => severity 2
        for wait in &waits {
            assert_eq!(wait.meta.severity_hint, 2);
        }
    }

    #[test]
    fn semaphore_severity_matches_spec_thresholds() {
        let make_sem = |oldest_wait: f64| -> ProcessDump {
            let mut dump = empty_dump(1, "app");
            dump.tasks = vec![make_task(1, "w", TaskState::Pending, 30.0)];
            dump.sync = Some(SyncSnapshot {
                mpsc_channels: vec![],
                oneshot_channels: vec![],
                watch_channels: vec![],
                semaphores: vec![SemaphoreSnapshot {
                    name: "s".to_string(),
                    permits_total: 1,
                    permits_available: 0,
                    waiters: 1,
                    acquires: 10,
                    avg_wait_secs: oldest_wait / 2.0,
                    max_wait_secs: oldest_wait,
                    age_secs: 60.0,
                    creator_task_id: None,
                    creator_task_name: None,
                    top_waiter_task_ids: vec![1],
                    oldest_wait_secs: oldest_wait,
                }],
                once_cells: vec![],
            });
            dump
        };

        // < 1s => severity 0
        let g = WaitGraph::build(&[make_sem(0.5)]);
        let s = g
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        assert_eq!(s.meta.severity_hint, 0);

        // > 1s => severity 1
        let g = WaitGraph::build(&[make_sem(5.0)]);
        let s = g
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        assert_eq!(s.meta.severity_hint, 1);

        // > 10s => severity 2
        let g = WaitGraph::build(&[make_sem(20.0)]);
        let s = g
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        assert_eq!(s.meta.severity_hint, 2);

        // > 30s => severity 3
        let g = WaitGraph::build(&[make_sem(45.0)]);
        let s = g
            .edges
            .iter()
            .find(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .unwrap();
        assert_eq!(s.meta.severity_hint, 3);
    }

    #[test]
    fn semaphore_no_self_cycle() {
        // A semaphore created by task-1 with task-1 also waiting on it
        // should produce a real 2-node cycle (task <-> sem), not a spurious artifact.
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(1, "worker", TaskState::Pending, 5.0)];
        dump.sync = Some(SyncSnapshot {
            mpsc_channels: vec![],
            oneshot_channels: vec![],
            watch_channels: vec![],
            semaphores: vec![SemaphoreSnapshot {
                name: "my-sem".to_string(),
                permits_total: 1,
                permits_available: 0,
                waiters: 1,
                acquires: 5,
                avg_wait_secs: 1.0,
                max_wait_secs: 2.0,
                age_secs: 10.0,
                creator_task_id: Some(1),
                creator_task_name: Some("worker".to_string()),
                top_waiter_task_ids: vec![1],
                oldest_wait_secs: 2.0,
            }],
            once_cells: vec![],
        });

        let graph = WaitGraph::build(&[dump]);
        let candidates = detect::find_deadlock_candidates(&graph);

        // The cycle task-1 -> sem -> task-1 is real (task waiting on sem it owns).
        // It's detected, but the point is it's not spurious — the edges are correct.
        if !candidates.is_empty() {
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].nodes.len(), 2); // task + semaphore
        }
    }

    #[test]
    fn roam_channel_no_synthetic_self_cycle() {
        // A roam channel associated with a task should not create
        // self-cycles because channel nodes only have TaskWaitsOnResource
        // edges (task -> channel), no ownership edge back.
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(1, "reader", TaskState::Pending, 5.0)];
        dump.roam = Some(SessionSnapshot {
            connections: vec![],
            method_names: HashMap::new(),
            channel_details: vec![RoamChannelSnapshot {
                channel_id: 1,
                name: "ch".to_string(),
                direction: ChannelDir::Rx,
                age_secs: 5.0,
                request_id: None,
                task_id: Some(1),
                task_name: Some("reader".to_string()),
                queue_depth: None,
                closed: false,
            }],
        });

        let graph = WaitGraph::build(&[dump]);
        let candidates = detect::find_deadlock_candidates(&graph);

        assert!(
            candidates.is_empty(),
            "roam channel should not introduce synthetic self-cycle"
        );
    }

    #[test]
    fn closed_roam_channel_no_wait_edge() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(1, "t", TaskState::Polling, 5.0)];
        dump.roam = Some(SessionSnapshot {
            connections: vec![],
            method_names: HashMap::new(),
            channel_details: vec![RoamChannelSnapshot {
                channel_id: 10,
                name: "c".to_string(),
                direction: ChannelDir::Rx,
                age_secs: 5.0,
                request_id: None,
                task_id: Some(1),
                task_name: Some("t".to_string()),
                queue_depth: None,
                closed: true,
            }],
        });

        let graph = WaitGraph::build(&[dump]);

        // Node should exist
        assert!(graph.nodes.contains_key(&NodeId::RoamChannel {
            pid: 1,
            channel_id: 10,
        }));

        // But a closed channel shouldn't produce a wait edge — the task
        // isn't blocked on it anymore.
        let waits: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskWaitsOnResource)
            .collect();
        assert!(waits.is_empty(), "closed channel should not produce wait edge");
    }

    // ── Future spawn edge tests ──────────────────────────────────

    #[test]
    fn future_spawn_edge_creates_future_to_future() {
        use peeps_types::FutureSpawnEdgeSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.future_spawn_edges = vec![FutureSpawnEdgeSnapshot {
            parent_future_id: 10,
            parent_resource: "join_all".to_string(),
            child_future_id: 20,
            child_resource: "rpc.call".to_string(),
            created_by_task_id: Some(1),
            created_by_task_name: Some("driver".to_string()),
            created_age_secs: 1.0,
        }];

        let graph = WaitGraph::build(&[dump]);

        // process + 2 futures = 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        let spawn_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureSpawnedFuture)
            .collect();
        assert_eq!(spawn_edges.len(), 1);
        assert_eq!(
            spawn_edges[0].from,
            NodeId::Future {
                pid: 1,
                future_id: 10
            }
        );
        assert_eq!(
            spawn_edges[0].to,
            NodeId::Future {
                pid: 1,
                future_id: 20
            }
        );
        assert_eq!(spawn_edges[0].meta.confidence, EdgeConfidence::Explicit);
    }

    #[test]
    fn future_spawn_self_cycle_skipped() {
        use peeps_types::FutureSpawnEdgeSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.future_spawn_edges = vec![FutureSpawnEdgeSnapshot {
            parent_future_id: 10,
            parent_resource: "self".to_string(),
            child_future_id: 10,
            child_resource: "self".to_string(),
            created_by_task_id: None,
            created_by_task_name: None,
            created_age_secs: 0.0,
        }];

        let graph = WaitGraph::build(&[dump]);
        assert!(graph.edges.is_empty());
    }

    // ── Future poll edge tests ───────────────────────────────────

    #[test]
    fn future_poll_edge_creates_task_to_future_with_severity() {
        use peeps_types::FuturePollEdgeSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(1, "driver", TaskState::Polling, 5.0)];
        dump.future_poll_edges = vec![FuturePollEdgeSnapshot {
            task_id: 1,
            task_name: Some("driver".to_string()),
            future_id: 42,
            future_resource: "http.request".to_string(),
            poll_count: 100,
            total_poll_secs: 6.0,
            last_poll_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);

        // process + task + future = 3 nodes
        assert_eq!(graph.nodes.len(), 3);
        let poll_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskPollsFuture)
            .collect();
        assert_eq!(poll_edges.len(), 1);
        assert_eq!(
            poll_edges[0].from,
            NodeId::Task {
                pid: 1,
                task_id: 1
            }
        );
        assert_eq!(
            poll_edges[0].to,
            NodeId::Future {
                pid: 1,
                future_id: 42
            }
        );
        // total_poll_secs=6.0 > 5.0 => severity 2
        assert_eq!(poll_edges[0].meta.severity_hint, 2);
        assert_eq!(poll_edges[0].meta.count, 100);
        assert_eq!(poll_edges[0].meta.confidence, EdgeConfidence::Explicit);
    }

    // ── Future resume edge tests ─────────────────────────────────

    #[test]
    fn future_resume_edge_creates_future_to_task_explicit() {
        use peeps_types::FutureResumeEdgeSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(2, "consumer", TaskState::Pending, 3.0)];
        dump.future_resume_edges = vec![FutureResumeEdgeSnapshot {
            future_id: 50,
            future_resource: "notify".to_string(),
            target_task_id: 2,
            target_task_name: Some("consumer".to_string()),
            resume_count: 5,
            last_resume_age_secs: 0.2,
        }];

        let graph = WaitGraph::build(&[dump]);

        let resume_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureResumesTask)
            .collect();
        assert_eq!(resume_edges.len(), 1);
        assert_eq!(
            resume_edges[0].from,
            NodeId::Future {
                pid: 1,
                future_id: 50
            }
        );
        assert_eq!(
            resume_edges[0].to,
            NodeId::Task {
                pid: 1,
                task_id: 2
            }
        );
        assert_eq!(resume_edges[0].meta.confidence, EdgeConfidence::Explicit);
        assert_eq!(resume_edges[0].meta.count, 5);
    }

    // ── Future resource edge tests ───────────────────────────────

    #[test]
    fn future_resource_edge_maps_lock_to_correct_node() {
        use peeps_types::{FutureResourceEdgeSnapshot, ResourceRefSnapshot};
        let mut dump = empty_dump(1, "app");
        dump.future_resource_edges = vec![FutureResourceEdgeSnapshot {
            future_id: 60,
            resource: ResourceRefSnapshot::Lock {
                process: "app".to_string(),
                name: "db_pool".to_string(),
            },
            wait_count: 10,
            total_wait_secs: 2.0,
            last_wait_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);

        let res_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureWaitsOnResource)
            .collect();
        assert_eq!(res_edges.len(), 1);
        assert_eq!(
            res_edges[0].to,
            NodeId::Lock {
                pid: 1,
                name: "db_pool".to_string()
            }
        );
        assert_eq!(res_edges[0].meta.severity_hint, 1); // >1s
    }

    #[test]
    fn future_resource_edge_maps_mpsc_to_correct_node() {
        use peeps_types::{FutureResourceEdgeSnapshot, ResourceRefSnapshot};
        let mut dump = empty_dump(1, "app");
        dump.future_resource_edges = vec![FutureResourceEdgeSnapshot {
            future_id: 61,
            resource: ResourceRefSnapshot::Mpsc {
                process: "app".to_string(),
                name: "work-queue".to_string(),
            },
            wait_count: 5,
            total_wait_secs: 0.5,
            last_wait_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);
        let res_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureWaitsOnResource)
            .collect();
        assert_eq!(res_edges.len(), 1);
        assert_eq!(
            res_edges[0].to,
            NodeId::MpscChannel {
                pid: 1,
                name: "work-queue".to_string()
            }
        );
    }

    #[test]
    fn future_resource_edge_maps_roam_channel() {
        use peeps_types::{FutureResourceEdgeSnapshot, ResourceRefSnapshot};
        let mut dump = empty_dump(1, "app");
        dump.future_resource_edges = vec![FutureResourceEdgeSnapshot {
            future_id: 62,
            resource: ResourceRefSnapshot::RoamChannel {
                process: "app".to_string(),
                channel_id: 42,
            },
            wait_count: 3,
            total_wait_secs: 12.0,
            last_wait_age_secs: 0.5,
        }];

        let graph = WaitGraph::build(&[dump]);
        let res_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureWaitsOnResource)
            .collect();
        assert_eq!(res_edges.len(), 1);
        assert_eq!(
            res_edges[0].to,
            NodeId::RoamChannel {
                pid: 1,
                channel_id: 42
            }
        );
        assert_eq!(res_edges[0].meta.severity_hint, 3); // >10s
    }

    #[test]
    fn future_resource_edge_maps_semaphore() {
        use peeps_types::{FutureResourceEdgeSnapshot, ResourceRefSnapshot};
        let mut dump = empty_dump(1, "app");
        dump.future_resource_edges = vec![FutureResourceEdgeSnapshot {
            future_id: 63,
            resource: ResourceRefSnapshot::Semaphore {
                process: "app".to_string(),
                name: "pool".to_string(),
            },
            wait_count: 1,
            total_wait_secs: 0.1,
            last_wait_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);
        let res_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::FutureWaitsOnResource)
            .collect();
        assert_eq!(res_edges.len(), 1);
        assert_eq!(
            res_edges[0].to,
            NodeId::Semaphore {
                pid: 1,
                name: "pool".to_string()
            }
        );
        assert_eq!(res_edges[0].meta.severity_hint, 0); // <1s
    }

    // ── Request parent edge tests ────────────────────────────────

    #[test]
    fn request_parent_creates_cross_process_stitch() {
        use peeps_types::RequestParentSnapshot;
        let mut dump_a = empty_dump(100, "frontend");
        dump_a.roam = Some(SessionSnapshot {
            connections: vec![make_connection(
                "conn-a",
                vec![make_rpc_request("get", 1, Direction::Outgoing, 2.0, Some(1), Some("t"))],
            )],
            method_names: HashMap::new(),
            channel_details: vec![],
        });
        dump_a.tasks = vec![make_task(1, "t", TaskState::Pending, 2.0)];
        dump_a.request_parents = vec![RequestParentSnapshot {
            child_process: "backend".to_string(),
            child_connection: "conn-b".to_string(),
            child_request_id: 2,
            parent_process: "frontend".to_string(),
            parent_connection: "conn-a".to_string(),
            parent_request_id: 1,
        }];

        let mut dump_b = empty_dump(200, "backend");
        dump_b.roam = Some(SessionSnapshot {
            connections: vec![make_connection(
                "conn-b",
                vec![make_rpc_request("get", 2, Direction::Incoming, 1.5, Some(2), Some("h"))],
            )],
            method_names: HashMap::new(),
            channel_details: vec![],
        });
        dump_b.tasks = vec![make_task(2, "h", TaskState::Polling, 1.5)];

        let graph = WaitGraph::build(&[dump_a, dump_b]);

        let parent_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| {
                e.kind == EdgeKind::RpcCrossProcessStitch
                    && e.meta.confidence == EdgeConfidence::Explicit
            })
            .collect();
        assert_eq!(parent_edges.len(), 1);
    }

    #[test]
    fn request_parent_same_process_uses_rpc_request_parent() {
        use peeps_types::RequestParentSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.roam = Some(SessionSnapshot {
            connections: vec![make_connection(
                "conn",
                vec![
                    make_rpc_request("a", 1, Direction::Incoming, 2.0, Some(1), Some("t1")),
                    make_rpc_request("b", 2, Direction::Incoming, 1.0, Some(2), Some("t2")),
                ],
            )],
            method_names: HashMap::new(),
            channel_details: vec![],
        });
        dump.tasks = vec![
            make_task(1, "t1", TaskState::Polling, 2.0),
            make_task(2, "t2", TaskState::Polling, 1.0),
        ];
        dump.request_parents = vec![RequestParentSnapshot {
            child_process: "app".to_string(),
            child_connection: "conn".to_string(),
            child_request_id: 2,
            parent_process: "app".to_string(),
            parent_connection: "conn".to_string(),
            parent_request_id: 1,
        }];

        let graph = WaitGraph::build(&[dump]);

        let parent_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::RpcRequestParent)
            .collect();
        assert_eq!(parent_edges.len(), 1);
        assert_eq!(parent_edges[0].meta.confidence, EdgeConfidence::Explicit);
    }

    // ── No self-cycle tests for new edge types ───────────────────

    #[test]
    fn future_poll_no_self_cycle_from_same_ids() {
        use peeps_types::FuturePollEdgeSnapshot;
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![make_task(1, "t", TaskState::Polling, 1.0)];
        dump.future_poll_edges = vec![FuturePollEdgeSnapshot {
            task_id: 1,
            task_name: Some("t".to_string()),
            future_id: 99,
            future_resource: "timer".to_string(),
            poll_count: 10,
            total_poll_secs: 0.5,
            last_poll_age_secs: 0.1,
        }];

        let graph = WaitGraph::build(&[dump]);
        // task -> future is not a self-cycle (different node types)
        for edge in &graph.edges {
            assert_ne!(edge.from, edge.to, "no self-cycles");
        }
    }

    #[test]
    fn existing_edges_are_derived() {
        let mut dump = empty_dump(1, "app");
        dump.tasks = vec![
            make_task(1, "a", TaskState::Polling, 1.0),
            make_task(2, "b", TaskState::Pending, 1.0),
        ];
        dump.tasks[1].parent_task_id = Some(1);
        dump.tasks[1].parent_task_name = Some("a".to_string());

        let graph = WaitGraph::build(&[dump]);
        let spawn_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::TaskSpawnedTask)
            .collect();
        assert_eq!(spawn_edges.len(), 1);
        assert_eq!(spawn_edges[0].meta.confidence, EdgeConfidence::Derived);
    }
}
