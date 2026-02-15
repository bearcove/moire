# Resource Track: Futures

Status: todo
Owner: wg-resource-futures
Priority: P0

## Mission

Represent instrumented futures as first-class nodes and explicit wait/wake/resume edges.

## Current context

- Future instrumentation is in `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/lib.rs` (`peepable`, future waits, future poll/resume/wake edges).
- Current `peepable` API is label-only; metadata needs to be added.

## Node + edge model

Node ID:
- `future:{process}:{pid}:{future_id}`

Node kind:
- `future`

Required attrs_json:
- `future_id`
- `label`
- `created_by_task_id`
- `last_polled_by_task_id`
- `pending_count`
- `ready_count`
- `total_pending_ns`
- `metadata_json` (arbitrary key/value metadata)

Required edges:
- `task_awaits_future` (`task -> future`)
- `task_wakes_future` (`task -> future`)
- `future_resumes_task` (`future -> task`)

## Implementation steps

1. Add metadata-capable API:
- `peepable_with_meta(future, label, metadata)`
- keep `peepable(label)` as convenience wrapper.
2. Persist metadata on future node attrs.
3. Emit only explicitly recorded wait/wake/resume edges.
4. Keep edge durations/counts in explicit fields (`duration_ns`, `count`, attrs overflow only for extra details).

## Consumer changes

Required:
- Add `peepable_with_meta` at important await points in Roam/Vixen:
  - request_id
  - method
  - channel_id
  - fd
  - path/resource key

## Validation SQL

```sql
SELECT COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'future';
```

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('task_awaits_future','task_wakes_future','future_resumes_task')
GROUP BY kind;
```
