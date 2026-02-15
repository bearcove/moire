# Resource Track: Watch

Status: todo
Owner: wg-resource-watch
Priority: P2

## Mission

Represent watch channels with explicit change and wait behavior.

## Current context

- Watch wrapper is in `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs` (`WatchInfo`, sender/receiver wrappers).
- `changed().await` wait behavior needs explicit edge coverage.

## Node + edge model

Node ID:
- `watch:{process}:{name}`

Node kind:
- `watch`

Required attrs_json:
- `name`
- `changes`
- `receiver_count`
- `age_ns`
- `creator_task_id`

Required edges:
- `task_sends_to_channel`
- `task_receives_from_channel`
- `task_waits_on_channel` for `changed().await`

## Implementation steps

1. Emit watch node each snapshot.
2. Emit send edge for updates.
3. Emit receive edge when receiver consumes change.
4. Emit wait edge with duration when receiver is blocked in `changed().await`.

## Consumer changes

- Transparent where wrappers are used.
- Migrate raw watch channels if any.

## Validation SQL

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1 AND kind = 'task_waits_on_channel';
```

(Manually verify watch waits are represented, not just mpsc waits.)
