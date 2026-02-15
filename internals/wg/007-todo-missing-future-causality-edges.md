# Missing Future Causality Edges (Request-Centric Wait Graph)

Status: todo
Owner: wg-causality
Scope: `peeps-tasks`, `peeps-types`, `peeps`, `peeps-waitgraph`, producers in Roam/Vixen

## Why

Today we can show:
- request chains (`chain/span/parent_span`)
- task parent/child
- task wake edges
- task -> future wake edges
- task waiting on future

But we cannot reliably prove `future A` causes `future B` (example: `rpc.client.call => channel.send.driver`) because the direct edges are missing.

## Missing Edges To Add

## 1) Future Spawn / Composition Edges

Need explicit future-to-future lineage:
- `FutureSpawnedBy`: `parent_future_id -> child_future_id`
- `FutureOwnedByTask`: `task_id -> root_future_id` (already partly inferable, keep explicit)

Use cases:
- async composition (`join!`, `select!`, driver loops)
- wrappers (`.peepable(...)`) preserving parent lineage

### Proposed snapshot types

```rust
pub struct FutureSpawnEdgeSnapshot {
    pub parent_future_id: FutureId,
    pub parent_resource: String,
    pub child_future_id: FutureId,
    pub child_resource: String,
    pub created_by_task_id: Option<TaskId>,
    pub created_by_task_name: Option<String>,
    pub created_age_secs: f64,
}
```

## 2) Future Poll Ownership Edges

Need to know who polled a future over time, not just "last polled":
- `TaskPollsFuture`: `task_id -> future_id` with count + last age + total poll time

This disambiguates:
- task-local wait
- executor handoff
- cross-task driving patterns

### Proposed snapshot type

```rust
pub struct FuturePollEdgeSnapshot {
    pub task_id: TaskId,
    pub task_name: Option<String>,
    pub future_id: FutureId,
    pub future_resource: String,
    pub poll_count: u64,
    pub total_poll_secs: f64,
    pub last_poll_age_secs: f64,
}
```

## 3) Future Wake Target Edges (Direct)

We have task -> future wake and optional target task in `FutureWakeEdgeSnapshot`, but we need normalized direct edge:
- `FutureWakesTask`: `future_id -> task_id`

This should be emitted as first-class data (not only inferred from optional target fields).

### Proposed snapshot type

```rust
pub struct FutureResumeEdgeSnapshot {
    pub future_id: FutureId,
    pub future_resource: String,
    pub target_task_id: TaskId,
    pub target_task_name: Option<String>,
    pub resume_count: u64,
    pub last_resume_age_secs: f64,
}
```

## 4) RPC Context -> Task Edge (Server Side)

Need stable bridge from request span to server task:
- `RequestSpanHandledByTask`: `span_id -> task_id`

This allows deterministic merge of request tree + task/future tree.

### Proposed payload extension

Add to request snapshot metadata and/or explicit field:
- `server_task_id`
- `server_task_name`
- `server_process`

(avoid relying on heuristics from method + timing).

## 5) Resource Wait Edges (Future -> Resource)

Right now `resource` is a string on `FutureWaitSnapshot`.
Need structured identity so graphs can join with Locks/Channels tabs:
- `FutureWaitsOnResource`: `future_id -> {kind, process, stable_id}`

### Proposed snapshot type

```rust
pub enum ResourceRefSnapshot {
    Lock { process: String, name: String },
    Mpsc { process: String, name: String },
    Oneshot { process: String, name: String },
    Watch { process: String, name: String },
    Semaphore { process: String, name: String },
    RoamChannel { process: String, channel_id: u64 },
    Socket { process: String, fd: u64, label: Option<String> },
    Unknown { label: String },
}

pub struct FutureResourceEdgeSnapshot {
    pub future_id: FutureId,
    pub resource: ResourceRefSnapshot,
    pub wait_count: u64,
    pub total_wait_secs: f64,
    pub last_wait_age_secs: f64,
}
```

## 6) Socket I/O Wait Edges

For deadlocks/stalls in real systems, add explicit socket wait edges:
- `FutureWaitsOnSocketReadable`
- `FutureWaitsOnSocketWritable`

Can be represented through `ResourceRefSnapshot::Socket` + reason metadata.

## 7) Cross-Process Request Parent Edge (Strict)

We currently infer via span ids.
Need explicit strict edge in payload:
- `RequestParent`: `{process, connection, request_id} -> {process, connection, request_id}`

This removes ambiguity when span metadata is partial.

## Integration Plan

## Phase A: Data model + snapshots
- Add new snapshot structs in `peeps-types`.
- Add collection in `peeps-tasks` (future lineage/poll/resume/resource edges).
- Thread through `peeps::Snapshot` and `ProcessDump`.

## Phase B: Producer instrumentation
- Roam/Vixen wrappers emit structured resource refs for waits.
- RPC server marks handling task on incoming request context.
- Socket wrappers emit readable/writable wait resources.

## Phase C: Waitgraph ingestion
- Ingest all new edges in `peeps-waitgraph`.
- Distinguish edge confidence:
  - `explicit` (emitted)
  - `derived` (computed)

## Phase D: UI usage
- Requests inline ELK graph uses explicit edges first.
- Visual style:
  - solid line = explicit
  - dashed line = derived

## Acceptance Criteria

1. For a sampled request, we can render:
   - request span -> task -> future -> resource
   - plus wake/resume edges
2. We can show at least one cross-process chain without heuristic matching.
3. For `rpc.client.call => channel.send.driver` scenarios, graph includes a concrete path with explicit edges only.
4. Deadlock candidates do not rely on self-cycle inference for single task+future pairs when explicit edges disprove a cycle.
5. Snapshot payload remains backward-compatible (new fields optional).

## Non-goals

- Perfect runtime provenance for all third-party futures without wrappers.
- Building a fully global live graph in browser memory.

## Notes

- Keep stable IDs monotonic and process-scoped.
- Prefer additive schema evolution.
- Preserve old consumers while new fields roll out.
