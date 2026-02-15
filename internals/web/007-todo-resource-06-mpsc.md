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

Node IDs:
- `mpsc:{process}:{name}:tx`
- `mpsc:{process}:{name}:rx`

Node kinds:
- `mpsc_tx`
- `mpsc_rx`

Required attrs_json (both endpoints):
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

Required `needs` edges:
- `task -> mpsc:{...}:tx` when task send-side progress depends on tx endpoint
- `task -> mpsc:{...}:rx` when task recv-side progress depends on rx endpoint
- `mpsc:{...}:tx -> mpsc:{...}:rx` to encode that producer-side progress depends on receiver draining

## Counter-to-edge translation (explicit mapping)

State counters stay on node attrs; causality must be emitted as `needs` edges:

- send-side dependency => `task -> ...:tx`
- recv-side dependency => `task -> ...:rx`
- channel internal dependency => `...:tx -> ...:rx`

`send_waiters`/`queue_len` are state metrics, not causal edges by themselves.

## Implementation steps

1. Add explicit occupancy counter in wrapper:
- increment on successful send
- decrement on successful recv
2. Track `high_watermark` from occupancy.
3. Populate `queue_len` from occupancy counter at snapshot time.
4. Emit `needs` edges to endpoint nodes (no send/recv edge kind variants).
5. Emit `tx -> rx` `needs` edge for each channel instance.
6. Keep consistent endpoint ID conventions globally.

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
SELECT src_id, dst_id
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND src_id LIKE 'mpsc:%:tx'
  AND dst_id LIKE 'mpsc:%:rx'
LIMIT 100;
```
