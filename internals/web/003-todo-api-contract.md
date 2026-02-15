# API Contract Spec (Raw SQL Endpoint)

Status: todo
Owner: wg-api
Scope: `crates/peeps-web` HTTP API

## Goal

Support point-in-time graph exploration with synchronized pull snapshots and one SQL endpoint.

## Endpoint (v1)

- `POST /api/sql`
  - request body:
```json
{
  "snapshot_id": 1234,
  "sql": "SELECT id, kind, process, attrs_json FROM nodes WHERE kind = ?1 LIMIT ?2",
  "params": ["request", 200]
}
```
  - response body:
```json
{
  "snapshot_id": 1234,
  "columns": ["id", "kind", "process", "attrs_json"],
  "rows": [
    ["request:vx-vfsd:...", "request", "vx-vfsd", "{\"elapsed_secs\": 8.1}"]
  ],
  "row_count": 1,
  "truncated": false
}
```

- `POST /api/jump-now`
  - triggers synchronized pull from all connected processes
  - request body (optional):
```json
{
  "timeout_ms": 1500
}
```
  - response body:
```json
{
  "snapshot_id": 1234,
  "requested_processes": 5,
  "responded_processes": 4,
  "timed_out_processes": 1
}
```

## Query policy (local-only but still constrained)

- read-only SQL only:
  - allow `SELECT`, `WITH`, `EXPLAIN QUERY PLAN`
  - reject `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`, `ATTACH`, `DETACH`, `PRAGMA`, multiple statements
- enforce single statement per request
- enforce statement timeout and max rows/bytes
- always parameterized (`?1`, `?2`, ...)

## Snapshot model

- `snapshot_id` in request fixes point-in-time view.
- no auto-updates; frontend issues explicit `POST /api/jump-now`.
- no snapshot picker in UI; flow is always "jump to now, then inspect".

## Result format guarantees

- stable column order as returned by SQLite
- values returned as JSON scalars/strings
- explicit truncation flag when row/size cap hit

## Error behavior

- validation / forbidden SQL: `400` with reason
- timeout / limit exceeded: `413` or `422` with reason
- DB failure: `500` with structured error JSON

## Performance SLO (dev)

- small/medium investigative query: < 100 ms
- larger graph slices: bounded by row/size caps with truncation
- no requirement to fetch full dump into browser by default

## Acceptance criteria

1. Frontend uses `POST /api/jump-now` to create synchronized snapshots.
2. Frontend reads data using `POST /api/sql` with `snapshot_id`.
3. No auto-update behavior; manual refresh only.
4. Large datasets remain usable via bounded SQL queries and truncation.
