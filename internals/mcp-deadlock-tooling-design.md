# MCP Deadlock Tooling Design (moire-web)

When an agent is debugging a hang, the current MCP surface forces too many round-trips:

- fetch a snapshot
- manually reconstruct wait edges
- manually chase entity IDs
- separately fetch source preview HTML

That is backwards for deadlock triage. The agent is traversing a runtime wait graph, so MCP needs to present graph-native, typed, deadlock-focused views with source context already attached in `text/plain` form.

## Goals

1. Make deadlock triage one-pass for agents.
2. Keep outputs strict and typed (no SQL and no query-pack indirection).
3. Reuse the existing tree-sitter source extraction pipeline.
4. Avoid returning HTML in MCP responses.
5. Support snapshot-to-snapshot comparisons from MCP.

## Non-goals

1. Replacing frontend HTTP APIs in this phase.
2. Adding write/update operations beyond coordinated cut capture.
3. Supporting arbitrary SQL from MCP.

## New MCP tool surface

### 1) `moire_cut_fresh`

Capture a fresh coordinated snapshot and return:

- `cut_id` (if cut trigger issued)
- `snapshot_id`
- `captured_at_unix_ms`
- `connected_processes`
- `timed_out_processes`

Notes:

- This is the anchor tool. Other tools can accept `snapshot_id` and work from the same cut.

### 2) `moire_wait_edges`

Return typed wait edges from one snapshot:

- `snapshot_id`
- `wait_edges[]`
  - `process_id`
  - `waiter_id`, `waiter_name`, `waiter_kind`
  - `blocked_on_id`, `blocked_on_name`, `blocked_on_kind`
  - `waiter_birth_ms`, `blocked_birth_ms`
  - `edge_kind` (must be `waiting_on`)
  - `waiter_source` (embedded source context, `text/plain`)

### 3) `moire_wait_chains`

Return precomputed chains over `waiting_on` edges:

- `snapshot_id`
- `chains[]`
  - `chain_id`
  - `node_ids[]`
  - `edges[]`
  - `is_cycle`
  - `has_external_wake_source`
  - `summary`
  - `nodes[]` with source snippets embedded

### 4) `moire_deadlock_candidates`

Return SCC/cycle-level deadlock candidates:

- `snapshot_id`
- `candidates[]`
  - `candidate_id`
  - `entity_ids[]`
  - `confidence` (`low|medium|high`)
  - `reasons[]`
  - `blocked_duration_hint_ms`
  - `cycle_nodes[]` with source snippets embedded

### 5) `moire_entity`

Return one entity with enriched context:

- `snapshot_id`
- `entity` (full typed entity record)
- `incoming_edges[]`
- `outgoing_edges[]`
- `scopes[]`
- `source` (embedded `text/plain` source context)

### 6) `moire_channel_state`

Channel-focused extraction for one entity (or all channel entities):

- `snapshot_id`
- `channels[]`
  - `entity_id`, `name`
  - `channel_kind`
  - `capacity`, `occupancy`
  - `receiver_waiters`, `sender_waiters`
  - `lifecycle_hints`
  - `source`

### 7) `moire_task_state`

Future/task-focused extraction:

- `snapshot_id`
- `tasks[]`
  - `entity_id`, `name`
  - `awaiting_on_entity_id`
  - `scope_ids[]`
  - `entry_backtrace_id`
  - `source`

### 8) `moire_source_context`

Direct source lookup still exists for ad-hoc use, but MCP returns plain text:

- `snapshot_id` (optional)
- `frame_ids[]`
- `format` (required; currently `text/plain`)
- `previews[]`
  - `frame_id`
  - `source_file`
  - `target_line`, `target_col`
  - `total_lines`
  - `statement_text`
  - `enclosing_fn_text`
  - `compact_scope_text`
  - `compact_scope_range`

Implementation detail: generated from existing tree-sitter path (`extract_target_statement`, `extract_enclosing_fn`, `cut_source_compact`).

### 9) `moire_backtrace`

Backtrace expansion with optional embedded source per frame:

- `snapshot_id`
- `backtrace_id`
- `frames[]`
  - resolved/unresolved frame payload
  - `source` (optional, plain text)

### 10) `moire_diff_snapshots`

Snapshot delta focused on deadlock triage:

- `from_snapshot_id`
- `to_snapshot_id`
- `entity_changes`
- `edge_changes`
- `waiting_on_changes`
- `channel_changes`
- `task_changes`

## Source context format for MCP

MCP must not return HTML fields. For MCP, source context is strictly text-first:

- `format = text/plain`
- statement-level snippet
- enclosing function signature text
- compact scope excerpt with stable line range

Frontend keeps existing HTML responses via HTTP APIs.

## Snapshot retention

Current state only retains the latest snapshot JSON. For MCP graph tooling we need recent history.

Server state will retain a bounded ring (default 64) of snapshots keyed by `snapshot_id`.

Required behaviors:

1. Exact lookup by snapshot id.
2. Fast latest lookup.
3. Strict error when requested snapshot id is absent.

## Migration and retirement

Phase 1:

- Add the 10 new MCP tools.
- Keep legacy MCP tools temporarily.

Phase 2:

- Remove legacy MCP tools:
  - `moire_sql_readonly`
  - `moire_query_pack`
  - HTML-centric source preview MCP output

Phase 3:

- Update agent prompt/docs to use graph-native tools by default.

## Strictness rules

1. Unknown `snapshot_id` is a hard error.
2. Unknown `entity_id` / `backtrace_id` / `frame_id` is a hard error.
3. Unsupported source `format` is a hard error.
4. No silent fallback to SQL or implicit latest snapshot when explicit snapshot id is provided.

## Initial implementation slice

This first implementation pass will:

1. Add snapshot history retention.
2. Add the new MCP tools and wire them end-to-end.
3. Ensure `moire_wait_edges`, `moire_wait_chains`, and `moire_deadlock_candidates` include embedded source context snippets.
4. Remove SQL and query-pack MCP tools from the exposed surface.
