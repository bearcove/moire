# Resource Track: SHM (Optional)

Status: todo
Owner: wg-resource-shm
Priority: P3

## Mission

Optionally map SHM diagnostics into canonical nodes/edges if needed for deadlock triage.

## Current context

- SHM diagnostics already exist (segments, peers, queues) via roam SHM snapshots.
- This track is optional because initial deadlock value may come from tasks/futures/rpc/sockets first.

## Node + edge model

Possible node IDs:
- `shm-segment:{process}:{segment_path}`
- `shm-peer:{process}:{peer_id}`
- `shm-queue:{process}:{name}`

Possible edges (explicit only):
- `shm_segment_has_peer`
- `task_uses_shm_queue`
- `request_uses_shm_queue`

Required attrs_json:
- segment capacity/usage
- peer state/heartbeat
- queue len/capacity

## Implementation steps

1. Build adapter from SHM diagnostic snapshots to canonical nodes.
2. Emit only explicitly measurable relationships.
3. Skip this track entirely if it does not improve request/deadlock workflows.

## Consumer changes

- None expected; mostly adapter-side work.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind LIKE 'shm%'
GROUP BY kind;
```
