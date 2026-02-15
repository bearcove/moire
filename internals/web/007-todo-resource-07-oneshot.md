# Resource Track: Oneshot

Status: todo
Owner: wg-resource-oneshot
Priority: P2

## Mission

Emit explicit oneshot lifecycle and task interaction data.

## Current context

- Oneshot wrapper is in `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs` (`OneshotInfo`, sender/receiver wrappers).
- Current state enum exists (pending/sent/received/dropped).

## Node + edge model

Node ID:
- `oneshot:{process}:{name}`

Node kind:
- `oneshot`

Required attrs_json:
- `name`
- `state`
- `age_ns`
- `creator_task_id`

Required edges:
- `task_sends_to_channel`
- `task_receives_from_channel`
- `task_waits_on_channel` (only when receive path blocks/waits)

## Implementation steps

1. Emit oneshot node per channel instance.
2. Emit explicit send edge on successful send.
3. Emit explicit receive edge on successful receive.
4. Emit wait edge with duration for pending receive waits.
5. Keep dropped-state transitions as node attrs, not fake edges.

## Consumer changes

- Transparent where wrapper constructors are used.
- Migrate raw oneshot usage where wrappers are bypassed.

## Validation SQL

```sql
SELECT json_extract(attrs_json, '$.state') AS state, COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'oneshot'
GROUP BY state;
```
