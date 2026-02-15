# Node/Edge Projection Spec

Status: todo
Owner: wg-projection
Scope: `crates/peeps-web` normalization layer

## Goal

Project `ProcessDump` into canonical nodes/edges with stable IDs.

Canonical wrapper emission API is defined in `peeps-types`:
- `GraphNodeSnapshot`
- `GraphEdgeSnapshot`
- `GraphSnapshotBuilder`
- `GraphEdgeOrigin::Explicit` (only allowed provenance)

## ID conventions (v1)

- Process: `process:{process}:{pid}`
- Task: `task:{process}:{pid}:{task_id}`
- Future: `future:{process}:{pid}:{future_id}`
- Request: `request:{process}:{pid}:{connection}:{request_id}`
- Lock: `lock:{process}:{name}`
- Channel: `mpsc:{process}:{name}`, `oneshot:{process}:{name}`, `watch:{process}:{name}`
- Semaphore: `semaphore:{process}:{name}`
- OnceCell: `oncecell:{process}:{name}`
- Roam channel: `roam-channel:{process}:{channel_id}`
- Socket (if present): `socket:{process}:{fd}`

## Required node kinds (v1)

- `process`
- `task`
- `future`
- `request`
- `resource` sub-kinds encoded in `kind`:
  - `lock`, `mpsc`, `oneshot`, `watch`, `semaphore`, `oncecell`, `roam_channel`, `socket`, `unknown`

## Required edge kinds (v1)

- topology
  - `task_in_process` (task -> process)
  - `request_in_process` (request -> process)
- causal
  - `request_handled_by_task` (request -> task)
  - `task_awaits_future` (task -> future)
  - `future_waits_on_resource` (future -> resource)
  - `task_sends_to_channel` (task -> channel)
  - `task_receives_from_channel` (task -> channel)
  - `task_waits_on_channel` (task -> channel)
  - `task_spawns_task` (parent task -> child task)
  - `task_wakes_task` (source task -> target task)
  - `task_wakes_future` (source task -> future)
  - `future_resumes_task` (future -> task)
  - `request_parent` (child request -> parent request)

## attrs_json contract

Use first-class columns for common edge fields (not buried in JSON):

- `edge_id` (stable id)
- `src_id`
- `dst_id`
- `kind`
- `process`
- `seq`
- `event_ns`
- `duration_ns` (nullable)
- `count` (default 1)
- `label` (nullable)
- `source_file` (nullable)
- `source_line` (nullable)
- `source_col` (nullable)

`edges.attrs_json` is only for optional type-specific overflow fields.

`edges` rows are explicit events only. No synthetic/derived/heuristic edges.

Example explicit edge payload fields:

```json
{
  "wait_secs": 0.0,
  "count": 1
}
```

Every `nodes.attrs_json` should include canonical keys per type (`task_id`, `name`, `state`, etc.).

For `mpsc` nodes, required keys are:
- `bounded`
- `capacity`
- `queue_len`
- `high_watermark`
- `utilization` (bounded only)
- `sender_count`
- `send_waiters`
- `sender_closed`
- `receiver_closed`
- `sent_total`
- `recv_total`

## Acceptance criteria

1. Same semantic relationship always maps to same node IDs and edge kinds.
2. Selected stuck request can be expanded to a connected subgraph using only canonical edge kinds.
3. All causal edges in storage come from explicit instrumentation data only.
