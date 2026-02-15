# Resource Track: Tokio Channels

Status: todo
Owner: wg-resource-tokio-channels
Priority: P0

## Mission

Treat tokio channels as one track and model all of them with endpoint nodes.

This track covers:
- mpsc
- oneshot
- watch

## Current context

- Channel wrappers are in `/Users/amos/bearcove/peeps/crates/peeps-sync/src/lib.rs`.
- Existing per-type snapshots exist, but canonical endpoint-node modeling is not unified yet.

## Node + edge model

Use endpoint nodes for all channel types:

- `mpsc:{process}:{name}:tx`, `mpsc:{process}:{name}:rx`
- `oneshot:{process}:{name}:tx`, `oneshot:{process}:{name}:rx`
- `watch:{process}:{name}:tx`, `watch:{process}:{name}:rx`

Node kinds:
- `mpsc_tx`, `mpsc_rx`, `oneshot_tx`, `oneshot_rx`, `watch_tx`, `watch_rx`

Required attrs_json (per endpoint, type-specific):
- common: `name`, `created_at_ns`, `creator_task_id`, closed flags
- mpsc: `bounded`, `capacity`, `queue_len`, `high_watermark`, `utilization`, `sender_count`, `send_waiters`, `sent_total`, `recv_total`
- oneshot: `state`, `age_ns`
- watch: `changes`, `receiver_count`, `age_ns`

Required `needs` edges:
- `task -> ...:tx` when task progress depends on send endpoint
- `task -> ...:rx` when task progress depends on recv endpoint
- `...:tx -> ...:rx` endpoint dependency for each channel instance

## Implementation steps

1. Refactor channel emitters to produce endpoint nodes for all three channel types.
2. Emit endpoint dependency edge (`tx -> rx`) for every channel instance.
3. Emit task->endpoint edges only from explicit measured interactions.
4. Keep channel health/state metrics on nodes, not edges.
5. For mpsc, maintain explicit occupancy + high watermark in wrapper state.

## Consumer changes

- Transparent where `peeps-sync` wrappers are used.
- Required migration for raw tokio channel callsites in consumers.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM nodes
WHERE snapshot_id = ?1
  AND kind IN ('mpsc_tx','mpsc_rx','oneshot_tx','oneshot_rx','watch_tx','watch_rx')
GROUP BY kind;
```

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND (src_id LIKE 'mpsc:%:tx' AND dst_id LIKE 'mpsc:%:rx'
       OR src_id LIKE 'oneshot:%:tx' AND dst_id LIKE 'oneshot:%:rx'
       OR src_id LIKE 'watch:%:tx' AND dst_id LIKE 'watch:%:rx');
```
