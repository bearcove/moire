# Resource Track: Semaphore

Status: todo
Owner: wg-resource-semaphore
Priority: P1

## Mission

Make semaphore contention and wait duration explicit.

## Current context

- Wrapper is `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs` (`DiagnosticSemaphore`, `SemaphoreInfo`).
- Current snapshot has waiters/acquires/avg/max but edge-level task waits must be explicit.

## Node + edge model

Node ID:
- `semaphore:{process}:{name}`

Node kind:
- `semaphore`

Required attrs_json:
- `name`
- `permits_total`
- `permits_available`
- `waiters`
- `acquires`
- `oldest_wait_ns`
- `high_waiters_watermark`
- `creator_task_id`

Required edges:
- `task_waits_on_semaphore` with measured `duration_ns`
- optional `task_acquires_semaphore` on acquire success

## Implementation steps

1. Instrument all acquire paths (borrowed + owned + try variants).
2. Emit wait edge only for actual waiting paths.
3. Keep try-acquire failures as explicit attrs/counters, not fake wait edges.
4. Track watermark metrics in wrapper state.

## Consumer changes

- Transparent where `DiagnosticSemaphore` is used.
- Migrate raw `tokio::sync::Semaphore` where present.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1 AND kind LIKE '%semaphore%'
GROUP BY kind;
```
