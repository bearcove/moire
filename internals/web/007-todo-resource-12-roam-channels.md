# Resource Track: Roam Channels

Status: todo
Owner: wg-resource-roam-channels
Priority: P1

## Mission

Expose roam channel usage as first-class resources tied to tasks and requests.

## Current context

- Roam diagnostics include channel details (`channel_id`, `direction`, `queue_depth`, task/request ids when present).
- Canonical node/edge mapping is not yet guaranteed for all channel events.

## Node + edge model

Node ID:
- `roam-channel:{process}:{channel_id}`

Node kind:
- `roam_channel`

Required attrs_json:
- `channel_id`
- `name`
- `direction`
- `queue_depth`
- `closed`
- `request_id`
- `task_id`

Required edges:
- `request_uses_channel` (`request -> roam_channel`)
- `task_sends_to_channel` (`task -> roam_channel`)
- `task_receives_from_channel` (`task -> roam_channel`)

## Implementation steps

1. Emit channel node for each channel detail entry.
2. Emit request linkage edge when `request_id` exists.
3. Emit task send/recv edges when task IDs are explicitly known.
4. Do not synthesize task/request links from names.

## Consumer changes

- Usually none if roam internals are instrumented centrally.
- Add missing instrumentation at internal roam channel ops if edge coverage is sparse.

## Validation SQL

```sql
SELECT COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'roam_channel';
```

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('request_uses_channel','task_sends_to_channel','task_receives_from_channel')
GROUP BY kind;
```
