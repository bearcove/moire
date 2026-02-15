# Resource Track: MPSC Channels

Status: todo
Owner: wg-resource-mpsc
Priority: P0

## Mission

Stop treating MPSC as opaque counters. Emit real buffer state and explicit task/channel interaction edges.

## Current context

- MPSC wrapper is in `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs` (`MpscInfo`, sender/receiver wrappers).
- Current fields include sent/received/waiters/sender_count but buffer internals are incomplete.

## Node + edge model

Node ID:
- `mpsc:{process}:{name}`

Node kind:
- `mpsc`

Required attrs_json:
- `name`
- `bounded`
- `capacity`
- `queue_len` (actual current occupancy)
- `high_watermark`
- `utilization` (bounded only)
- `sender_count`
- `send_waiters`
- `sender_closed`
- `receiver_closed`
- `sent_total`
- `recv_total`
- `created_at_ns`
- `creator_task_id`

Required edges:
- `task_sends_to_channel` (`task -> mpsc`)
- `task_receives_from_channel` (`task -> mpsc`)
- `task_waits_on_channel` (`task -> mpsc`) with blocked wait duration when send/recv blocks

## Implementation steps

1. Add explicit occupancy counter in wrapper:
- increment on successful send
- decrement on successful recv
2. Track `high_watermark` from occupancy.
3. Populate `queue_len` from occupancy counter at snapshot time.
4. Emit send/recv edges with counts.
5. Emit wait edges for blocking operations with measured `duration_ns`.
6. Keep consistent edge direction and edge kind names.

## Consumer changes

- Transparent only where `peeps-sync` MPSC wrappers are used.
- Required migration for remaining raw `tokio::sync::mpsc` callsites in Roam/Vixen.

## Validation SQL

```sql
SELECT id, json_extract(attrs_json, '$.queue_len') AS q, json_extract(attrs_json, '$.capacity') AS cap
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'mpsc'
ORDER BY q DESC;
```

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('task_sends_to_channel','task_receives_from_channel','task_waits_on_channel')
GROUP BY kind;
```
