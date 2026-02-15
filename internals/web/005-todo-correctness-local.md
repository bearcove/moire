# Correctness (Local Dev)

Status: todo
Owner: wg-quality
Scope: local correctness checks only (single developer workflow)

## Goal

Ensure graph data is correct and explicit on a local machine.
No perf benchmarking. No rollout plan.

## Local correctness checks

1. Snapshot synchronization
- `Jump to now` creates one `snapshot_id`.
- Connected processes respond into that same `snapshot_id`.
- Missing responders are marked with explicit status.

2. Atomic writes
- Process reply write is transactional.
- Failed ingest does not leave partial rows for that process reply.

3. Canonical identity
- Node IDs follow conventions exactly.
- Edge source/destination IDs point to existing nodes in same snapshot.

4. Explicit-only edges
- All stored causal edges are explicit instrumentation events.
- No inferred/derived/heuristic edges in storage.

5. Resource-track completeness
- For each completed `007-*` track:
  - required node kind appears
  - required attrs exist
  - required edge kinds appear

## Quick local validation queries

```sql
-- Missing node references in edges
SELECT e.kind, e.src_id, e.dst_id
FROM edges e
LEFT JOIN nodes ns ON ns.snapshot_id = e.snapshot_id AND ns.id = e.src_id
LEFT JOIN nodes nd ON nd.snapshot_id = e.snapshot_id AND nd.id = e.dst_id
WHERE e.snapshot_id = ?1 AND (ns.id IS NULL OR nd.id IS NULL)
LIMIT 50;
```

```sql
-- Node counts by kind
SELECT kind, COUNT(*)
FROM nodes
WHERE snapshot_id = ?1
GROUP BY kind
ORDER BY COUNT(*) DESC;
```

```sql
-- Edge counts by kind
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
GROUP BY kind
ORDER BY COUNT(*) DESC;
```

## Acceptance criteria

1. Local stuck-request workflow is reliable from one `Jump to now` snapshot.
2. Edge/node integrity checks pass on live local runs.
3. No heuristic edges exist in stored snapshots.
