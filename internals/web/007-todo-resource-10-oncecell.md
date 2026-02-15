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

Required edges:
- `task_initializes_oncecell`
- `task_waits_on_oncecell`

## Implementation steps

1. Emit oncecell node each snapshot.
2. Emit init edge when task enters initializer closure.
3. Emit wait edge when task waits for another initializer to finish.
4. Use explicit task IDs only; no guessed actor linking.

## Consumer changes

- Transparent where wrapper type is used.
- Migrate raw once-cell use where missing.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1 AND kind IN ('task_initializes_oncecell','task_waits_on_oncecell')
GROUP BY kind;
```
