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

Required edges:
- `task_in_process` (`task -> process`)
- `task_spawns_task` (`parent_task -> child_task`)
- `task_wakes_task` (`source_task -> target_task`)

## Implementation steps

1. Add canonical task node emission in `peeps-tasks` graph builder path.
2. Emit `task_in_process` for each task node.
3. Emit `task_spawns_task` only when parent task ID is explicitly known.
4. Emit `task_wakes_task` from recorded wake events only.
5. Do not create synthetic parent/wake edges for missing data.

## Consumer changes

Required where missing instrumentation:
- Replace `tokio::spawn(...)` with `peeps_tasks::spawn_tracked(...)` in critical paths.
- Start with Roam + Vixen entry points where deadlocks are investigated.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('task_in_process', 'task_spawns_task', 'task_wakes_task')
GROUP BY kind;
```

Sanity check:
- `task_in_process` count ~= `task` node count.
