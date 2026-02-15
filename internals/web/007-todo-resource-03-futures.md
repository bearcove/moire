# Resource Track: Futures

Status: todo
Owner: wg-resource-futures
Priority: P0

## Mission

Represent instrumented futures as first-class nodes with explicit `needs` dependencies.

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

Required `needs` edges:
- `task -> future` (task progress depends on future progress)
- `future -> task` only when explicitly measured as a wake/resume dependency
- `future -> resource` only when explicitly measured

## Implementation steps

1. Add metadata-capable API:
- `peepable_with_meta(future, label, metadata)`
- keep `peepable(label)` as convenience wrapper.
2. Persist metadata on future node attrs.
3. Emit only explicitly recorded `needs` dependencies.
4. Do not require duration/count semantics on edges.

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
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND (
    (src_id LIKE 'task:%' AND dst_id LIKE 'future:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'task:%')
  );
```
