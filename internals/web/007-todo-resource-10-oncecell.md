# Resource Track: OnceCell

Status: todo
Owner: wg-resource-oncecell
Priority: P2

## Mission

Capture once-cell initialization and waiting behavior explicitly.

## Current context

- Wrapper is `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs` (`OnceCellInfo`, wrapper around `tokio::sync::OnceCell`).
- Existing snapshot has state/init duration, but edge-level wait/init actor linkage needs canonical emission.

## Node + edge model

Node ID:
- `oncecell:{process}:{name}`

Node kind:
- `oncecell`

Required attrs_json:
- `name`
- `state` (`empty|initializing|initialized`)
- `age_ns`
- `init_duration_ns`

Required `needs` edges:
- `task -> oncecell` when task depends on initialization completion

## Implementation steps

1. Emit oncecell node each snapshot.
2. Emit `task -> oncecell` `needs` edge on explicit init/wait dependency paths.
3. Use explicit task IDs only; no guessed actor linking.

## Consumer changes

- Transparent where wrapper type is used.
- Migrate raw once-cell use where missing.

## Validation SQL

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND src_id LIKE 'task:%'
  AND dst_id LIKE 'oncecell:%';
```
