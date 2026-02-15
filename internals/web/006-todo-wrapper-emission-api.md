# Wrapper Emission API Spec

Status: todo
Owner: wg-wrapper-api
Scope: `peeps-types` + wrapper crates (`peeps-tasks`, `peeps-locks`, `peeps-sync`, `peeps-threads`, roam diagnostics)

## Goal

All instrumentation wrappers emit canonical graph rows through one API.
No inferred/derived/heuristic edges. Ever.

## Canonical API (now)

Defined in `peeps-types`:

- `GraphNodeSnapshot`
- `GraphEdgeSnapshot`
- `GraphSnapshot`
- `GraphSnapshotBuilder`
- `GraphEdgeOrigin::Explicit` (single allowed origin)

## Required contract

1. Every resource becomes a node with stable `id` and `kind`.
2. Every relationship is an explicit measured edge.
3. `attrs_json` holds type-specific fields only.
4. Common query fields stay as top-level columns (id/src/dst/kind/process/duration/event/source location).
5. If an edge cannot be measured directly, it must not be emitted.

## Stable ID conventions (v1)

- process: `process:{process}:{pid}`
- task: `task:{process}:{pid}:{task_id}`
- thread: `thread:{process}:{thread_name}`
- future: `future:{process}:{pid}:{future_id}`
- request: `request:{process}:{pid}:{connection}:{request_id}`
- lock: `lock:{process}:{name}`
- semaphore: `semaphore:{process}:{name}`
- mpsc: `mpsc:{process}:{name}`
- oneshot: `oneshot:{process}:{name}`
- watch: `watch:{process}:{name}`
- oncecell: `oncecell:{process}:{name}`
- roam channel: `roam-channel:{process}:{channel_id}`
- socket: `socket:{process}:{fd}`

## Wrapper integration points

### peeps-tasks

Emit nodes:
- task
- future

Emit edges:
- task_in_process
- task_spawns_task
- task_awaits_future
- task_wakes_task
- task_wakes_future
- future_resumes_task

### peeps-locks

Emit nodes:
- lock

Emit edges:
- task_waits_on_lock
- lock_held_by_task

### peeps-sync

Emit nodes:
- mpsc / oneshot / watch / semaphore / oncecell

Emit edges:
- task_waits_on_channel
- task_sends_to_channel
- task_receives_from_channel
- task_waits_on_semaphore

#### Mandatory MPSC buffer state (no blind channels)

`mpsc` node `attrs_json` must include:
- `bounded` (bool)
- `capacity` (u64|null)
- `queue_len` (u64) at snapshot time
- `high_watermark` (u64) since channel creation
- `utilization` (0.0..1.0, bounded only)
- `sender_count` (u64)
- `send_waiters` (u64)
- `receiver_closed` (bool)
- `sender_closed` (bool)
- `sent_total` (u64)
- `recv_total` (u64)
- `created_at_ns` (u64)

Additionally, `peeps-sync` must emit explicit channel edges with measured timing/count:
- `task_sends_to_channel` (task -> mpsc)
- `task_receives_from_channel` (task -> mpsc)
- `task_waits_on_channel` (task -> mpsc, with `duration_ns` when blocked)

If bounded channel internals cannot provide exact `queue_len`, wrapper must maintain an explicit atomic occupancy counter at wrapper boundaries (increment on successful send, decrement on successful recv). No estimation from `sent-recv` at query time.

### peeps-threads

Emit nodes:
- thread

Emit edges:
- thread_runs_task (only when directly measured)
- thread_blocks_on_resource (only when directly measured)

### roam diagnostics

Emit nodes:
- request
- roam-channel
- socket (when fd is known)

Emit edges:
- request_handled_by_task
- request_parent
- request_waits_on_request (cross-process chain when metadata explicitly carries parent)
- request_uses_channel
- request_uses_socket

## ProcessDump migration

`ProcessDump.graph: Option<GraphSnapshot>` is the migration bridge.
During migration we may keep legacy typed sections, but `graph` is source-of-truth for `peeps-web`.

## Hard validation rules
 
At ingest (`peeps-web`) reject or quarantine edges when:
- `origin != Explicit`
- `src_id` missing node in same snapshot
- `dst_id` missing node in same snapshot
- unknown/empty `kind`

At CI level fail if any code path constructs non-explicit causal edges.

## Acceptance criteria

1. Each wrapper crate emits canonical nodes/edges via `GraphSnapshotBuilder`.
2. `peeps-web` can build request-centered subgraphs from canonical rows only.
3. No code path in repo emits inferred/derived/heuristic edges.
