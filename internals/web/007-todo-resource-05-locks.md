# Resource Track: Locks (Mutex/RwLock)

Status: todo
Owner: wg-resource-locks
Priority: P1

## Mission

Make lock ownership and lock waiting explicit in canonical graph.

## Current context

- Lock wrappers are in `/Users/amos/bearcove/peeps/crates/peeps-locks/src/lib.rs`.
- Existing lock snapshot includes holders/waiters and task IDs.

## Node + edge model

Node ID:
- `lock:{process}:{name}`

Node kind:
- `lock`

Required attrs_json:
- `name`
- `kind` (`mutex|rwlock_read|rwlock_write` where applicable)
- `acquires`
- `releases`
- `holder_count`
- `waiter_count`

Required edges:
- `lock_held_by_task` (`lock -> task` or `task -> lock`; pick one canonical direction and keep it global)
- `task_waits_on_lock` (`task -> lock`) with measured wait duration

## Implementation steps

1. Emit one lock node per tracked lock.
2. Emit explicit holder edges from holder records.
3. Emit explicit waiter edges from waiter records with wait duration.
4. Never derive holder from waiter or vice versa.

## Consumer changes

- Transparent only where wrapper lock types are used.
- Required migration where raw lock types bypass wrappers.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('lock_held_by_task','task_waits_on_lock')
GROUP BY kind;
```
