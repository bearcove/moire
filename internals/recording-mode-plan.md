# Recording Mode Plan

This file is the single source of truth for recording-mode work tracking.

Scope:
- Turn snapshotting into a record flow (`start`, periodic capture, `stop`)
- Support timeline scrubbing in UI
- Reach a final data model that supports stable layout over time

## Outcome We Want

When debugging transient stalls and recoveries, we can hit `Record`, let the app run, hit `Stop`, and scrub through time without losing graph readability.

## Milestones

### M0 - Groundwork

- [ ] Confirm product semantics for record sessions (`start`/`stop`, single active session, interval defaults)
- [ ] Confirm retention policy (frame cap and eviction behavior)
- [ ] Confirm whether recording is global (all connected processes) or filterable (future)
- [ ] Confirm minimum UI behavior while recording (live latest frame + elapsed + frame count)

### M1 - Record V1 (Thin Slice)

- [ ] Backend: add recording session state (`session_id`, `interval_ms`, `started_at`, `stopped_at`, `status`)
- [ ] Backend: periodic snapshot loop every `interval_ms` (default `500ms`)
- [ ] Backend: store frames in order with capture timestamp
- [ ] Backend: add APIs:
- [ ] `POST /api/record/start`
- [ ] `POST /api/record/stop`
- [ ] `GET /api/record/current`
- [ ] Frontend: add `Record`/`Stop` button
- [ ] Frontend: add basic timeline scrubber over captured frames
- [ ] Frontend: render selected frame as normal graph (no layout stabilization yet)
- [ ] Frontend: add `Live` toggle to follow newest frame while recording

### M2 - Final Data Model (Historical Union + Stable Layout)

- [ ] Backend: build session-level union graph (`all nodes/edges ever seen`)
- [ ] Backend: compute activity intervals per node/edge
- [ ] Backend: expose union graph + frame-indexed visibility metadata
- [ ] Frontend: run ELK on union graph (or backend-provided layout)
- [ ] Frontend: apply visibility masks when scrubbing (show only active at frame `t`)
- [ ] Frontend: keep stable positions across frames

### M3 - Temporal Diagnostics UX

- [ ] Add change summary per frame (`nodes +/-, edges +/-`)
- [ ] Add inspector diffs against previous frame
- [ ] Add optional ghost mode (dim non-active nodes)
- [ ] Add jump controls (`next change`, `prev change`)

### M4 - Scale + Export

- [ ] Add frame downsampling options for long sessions
- [ ] Add max memory guardrails + overflow behavior
- [ ] Add export/import for recording sessions
- [ ] Add perf telemetry for recording overhead

## Final Data Model (Target)

This is the target model we should design toward, even if V1 only implements a subset.

### RecordingSession

- `session_id: string`
- `status: "recording" | "stopped"`
- `interval_ms: u32`
- `started_at_unix_ms: i64`
- `stopped_at_unix_ms: i64 | null`
- `frame_count: u32`
- `max_frames: u32`
- `overflowed: bool`

### Frame

- `frame_index: u32` (monotonic, 0-based)
- `captured_at_unix_ms: i64`
- `snapshot_id: i64` (if backed by existing snapshot API)
- `processes: ProcessFrame[]`

### ProcessFrame

- `proc_key: string`
- `proc_time_ms: u64 | null`
- `snapshot: Snapshot` (existing peeps snapshot payload for that process)

### UnionGraph

- `nodes: NodeHistory[]`
- `edges: EdgeHistory[]`

### NodeHistory

- `node_id: string` (stable entity id)
- `kind: string`
- `first_seen_frame: u32`
- `last_seen_frame: u32`
- `active_intervals: Interval[]` (`[start, end]`, inclusive, can have gaps)
- `attrs_latest: object`
- `attrs_by_frame?: map<u32, object>` (optional; enable only if needed for rich temporal attr diffs)
- `layout?: { x: f32, y: f32, w: f32, h: f32 }`

### EdgeHistory

- `edge_id: string` (stable, deterministic from `from+to+kind+label` or explicit id)
- `from_node_id: string`
- `to_node_id: string`
- `kind: string` (for example `needs`, `touches`)
- `first_seen_frame: u32`
- `last_seen_frame: u32`
- `active_intervals: Interval[]`
- `attrs_latest: object`
- `layout?: { points: [f32, f32][] }`

### Interval

- `start_frame: u32`
- `end_frame: u32`

## API Shape (Planned)

- [ ] `POST /api/record/start`
  - request: `{ interval_ms?: number, max_frames?: number }`
  - response: `{ session_id, interval_ms, started_at_unix_ms, status }`
- [ ] `POST /api/record/stop`
  - request: `{ session_id }` (or implicit current)
  - response: `{ session_id, stopped_at_unix_ms, frame_count, status }`
- [ ] `GET /api/record/current`
  - response: current session metadata + frame list
- [ ] `GET /api/record/:session_id/frame/:frame_index`
  - response: frame data (V1 path)
- [ ] `GET /api/record/:session_id/union`
  - response: union graph + intervals (+ layout if precomputed)

## UI Interaction Model (Planned)

- [ ] `Record` button starts session and switches UI to recording mode
- [ ] `Stop` ends session and keeps timeline available
- [ ] Scrubber selects frame index
- [ ] `Live` mode auto-follows newest frame while recording
- [ ] Frame label shows absolute timestamp + relative elapsed
- [ ] Optional ghost toggle for non-active nodes

## Risks

- [ ] Layout jitter if we re-run ELK per frame instead of union graph
- [ ] Memory growth for long sessions with full attr history
- [ ] Snapshot latency drift at small intervals
- [ ] UX overload if we show too much temporal detail by default

## Decisions Log

- [x] Recording default interval target is `500ms`
- [ ] Decide whether to allow only one active session at a time
- [ ] Decide whether to compute layout in backend, frontend, or both
- [ ] Decide when union graph is built (on stop vs incremental during recording)
