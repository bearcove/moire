# API Contract Spec

Status: todo
Owner: wg-api
Scope: `crates/peeps-web` HTTP API

## Goal

Support manual, synchronized, local investigation.

## Endpoints (v1)

- `POST /api/jump-now`
  - triggers synchronized snapshot pull
  - returns:
    - `snapshot_id`
    - requested/responded/timed_out counts

- `POST /api/sql`
  - executes read-only SQL against selected snapshot
  - request:
```json
{
  "snapshot_id": 1234,
  "sql": "SELECT id, kind, process, attrs_json FROM nodes WHERE snapshot_id = ?1 LIMIT 200",
  "params": [1234]
}
```
  - response:
```json
{
  "snapshot_id": 1234,
  "columns": ["id", "kind", "process", "attrs_json"],
  "rows": [],
  "row_count": 0,
  "truncated": false
}
```

## SQL policy

Allow only read-only statements:
- `SELECT`
- `WITH`
- `EXPLAIN QUERY PLAN`

Reject mutations and dangerous operations:
- `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`, `ATTACH`, `DETACH`, `PRAGMA`
- multiple statements

## UX contract implications

- no snapshot picker required in UI
- user flow: click `Jump to now`, then inspect that snapshot
- no auto-refresh

## Acceptance criteria

1. `jump-now` creates synchronized snapshots.
2. `sql` queries are read-only and bounded.
3. `snapshot_id` is explicit in every query/response.
