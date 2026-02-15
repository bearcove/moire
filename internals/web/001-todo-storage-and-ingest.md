# Storage And Ingest Spec

Status: todo
Owner: wg-storage-ingest
Scope: `crates/peeps-web`

## Goal

Persist synchronized multi-process graph snapshots keyed by `snapshot_id`.

## Ingest contract (v1)

Server-orchestrated pull snapshots:

1. UI triggers `POST /api/jump-now`.
2. Server allocates `snapshot_id`.
3. Server requests dumps from all currently connected processes.
4. Each process replies with framed UTF-8 JSON payload tagged with `snapshot_id`.

Frame format:
- header: 4-byte big-endian length
- body: UTF-8 JSON `ProcessDump`

## SQLite schema (v1)

```sql
CREATE TABLE snapshots (
  snapshot_id INTEGER PRIMARY KEY,
  requested_at_ns INTEGER NOT NULL,
  completed_at_ns INTEGER,
  timeout_ms INTEGER NOT NULL
);

CREATE TABLE snapshot_processes (
  snapshot_id INTEGER NOT NULL,
  process TEXT NOT NULL,
  pid INTEGER,
  status TEXT NOT NULL, -- responded|timeout|disconnected|error
  recv_at_ns INTEGER,
  error_text TEXT,
  PRIMARY KEY (snapshot_id, process)
);

CREATE TABLE nodes (
  snapshot_id INTEGER NOT NULL,
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  process TEXT NOT NULL,
  attrs_json TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, id)
);

CREATE TABLE edges (
  snapshot_id INTEGER NOT NULL,
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  kind TEXT NOT NULL, -- always 'needs'
  attrs_json TEXT NOT NULL,
  PRIMARY KEY (snapshot_id, src_id, dst_id, kind)
);

CREATE INDEX idx_nodes_snapshot_kind ON nodes(snapshot_id, kind);
CREATE INDEX idx_nodes_snapshot_process ON nodes(snapshot_id, process);
CREATE INDEX idx_edges_snapshot_src ON edges(snapshot_id, src_id);
CREATE INDEX idx_edges_snapshot_dst ON edges(snapshot_id, dst_id);
```

## Write semantics

- `snapshot_id` is global and monotonic.
- Process replies are written transactionally per reply.
- Missing responders are represented in `snapshot_processes`.

## Retention

- Keep latest N snapshots (default 500).
- Delete old rows from `edges`, `nodes`, `snapshot_processes`, `snapshots` by `snapshot_id` cutoff.

## Acceptance criteria

1. `jump-now` yields one coherent `snapshot_id` across processes.
2. Missing process responses are explicit in status table.
3. No partial write for an individual process reply transaction.
