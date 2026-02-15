# Future Lineage And Poll Ownership Edges

Status: todo
Owner: wg-futures
Scope: `peeps-tasks`, `peeps-types`, `peeps`, `peeps-waitgraph`

## Why

We can observe task->future waits, but we cannot reconstruct future composition chains reliably.

## Deliverables

## 1) Future lineage edges

Add explicit future parent/child edges.

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

## 2) Poll ownership edges

Track who polled which future and how often.

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

## 3) Explicit future->task resume edges

Normalize future wake targets as first-class edges.

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

## Implementation Notes

- Preserve existing `future_waits` and `future_wake_edges`; add new snapshots alongside.
- Keep IDs process-scoped and monotonic.
- Record edges in wrappers (`peepable` path) and runtime hooks where available.

## Acceptance Criteria

1. A single request that involves nested futures renders a multi-hop future chain.
2. Graph can distinguish creator task vs polling task.
3. Resume edges are explicit (not inferred from optional target fields only).
