# Resource Track: Process

Status: todo
Owner: wg-resource-process
Priority: P1

## Mission

Emit one canonical process node per process dump so every other node has a stable process anchor.

## Current context

- `ProcessDump` is assembled in `/Users/amos/bearcove/peeps/crates/peeps/src/lib.rs`.
- Canonical graph envelope exists in `/Users/amos/bearcove/peeps/crates/peeps-types/src/lib.rs` (`GraphSnapshot`, `GraphNodeSnapshot`, `GraphEdgeSnapshot`).

## Node + edge model

Node ID:
- `process:{process}:{pid}`

Node kind:
- `process`

Required attrs_json:
- `pid`
- `process_name`
- `timestamp` (or `timestamp_ns` if available)

Edges:
- none emitted by this track directly.
- other tracks may point to process via `*_in_process` edges.

## Implementation steps

1. In `peeps` dump assembly, ensure process node is always emitted in `ProcessDump.graph`.
2. Ensure process node is emitted even if all other sections are empty.
3. If process metadata is duplicated elsewhere, canonical source must still be node attrs.

## Consumer changes

- None.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'process';
```

Expected: one process node per responding process in that snapshot.
