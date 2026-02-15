# Resource Track: Threads

Status: todo
Owner: wg-resource-threads
Priority: P2

## Mission

Emit thread nodes and only strictly measured thread causal edges.

## Current context

- Thread sampling is in `/Users/amos/bearcove/peeps/crates/peeps-threads/src/lib.rs`.
- Sampling currently provides stack/backtrace and stuckness signals.
- Thread->task/resource links are often not directly measurable; avoid guessing.

## Node + edge model

Node ID:
- `thread:{process}:{thread_name}`

Node kind:
- `thread`

Required attrs_json:
- `thread_name`
- `samples`
- `same_location_count`
- `dominant_frame`
- `backtrace` (optional)

Edges (only when explicitly known):
- `thread_runs_task`
- `thread_blocks_on_resource`

## Implementation steps

1. Emit thread nodes from sampling snapshot.
2. Emit no thread causal edges unless source data directly identifies both endpoints.
3. Do not infer thread->task from name matching or heuristics.

## Consumer changes

- None.

## Validation SQL

```sql
SELECT COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'thread';
```

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1 AND kind LIKE 'thread_%';
```
