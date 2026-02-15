# Resource Track: Tasks

Status: todo
Owner: wg-resource-tasks
Priority: P0

## Mission

Make task lifecycle and task-to-task causality first-class in canonical graph data.

## Current context

- Task tracking lives in `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/lib.rs`.
- Existing snapshots include `TaskSnapshot` and wake edges.
- `spawn_tracked` exists, but not all callsites in consumers necessarily use it.

## Node + edge model

Node ID:
- `task:{process}:{pid}:{task_id}`

Node kind:
- `task`

Required attrs_json:
- `task_id`
- `name`
- `state` (`pending|polling|completed`)
- `spawned_at_ns` (or equivalent)
- `parent_task_id`
- `spawn_backtrace` (optional if unavailable)

Required `needs` edges:
- `task -> task` for explicit spawn dependency
- `task -> task` for explicit wake dependency

## Implementation steps

1. Add canonical task node emission in `peeps-tasks` graph builder path.
2. Emit parent->child `needs` only when parent task ID is explicitly known.
3. Emit source->target `needs` for wake events only from explicit wake records.
4. Do not create synthetic parent/wake edges for missing data.

## Consumer changes

Required where missing instrumentation:
- Replace `tokio::spawn(...)` with `peeps_tasks::spawn_tracked(...)` in critical paths.
- Start with Roam + Vixen entry points where deadlocks are investigated.

## Validation SQL

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND src_id LIKE 'task:%'
  AND dst_id LIKE 'task:%';
```
