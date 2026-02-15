# Resource Track: RPC Requests

Status: todo
Owner: wg-resource-rpc-requests
Priority: P0

## Mission

Make request causality across tasks/processes explicit and queryable.

## Current context

- Roam session diagnostics surfaces in-flight requests and metadata.
- `peeps` currently extracts some request-parent data from metadata.
- Cross-process causality quality depends on robust metadata propagation.

## Node + edge model

Node ID:
- `request:{process}:{pid}:{connection}:{request_id}`

Node kind:
- `request`

Required attrs_json:
- `request_id`
- `method`
- `method_id`
- `direction` (`incoming|outgoing`)
- `elapsed_ns`
- `connection`
- `peer`
- `metadata_json`
- `args_json` (if available)

Required edges:
- `request_in_process` (`request -> process`)
- `request_handled_by_task` (`request -> task`)
- `request_parent` (`child_request -> parent_request`) only when explicit propagation metadata exists

## Implementation steps

1. Ensure request nodes include task linkage fields when known.
2. Emit `request_handled_by_task` only when task ID is explicitly provided.
3. Emit `request_parent` only from explicit propagated context.
4. Keep chain/span identifiers as attrs only; no inferred parent linking.

## Consumer changes

Required:
- Ensure all outbound RPC call sites propagate parent request context metadata.
- Ensure server-side handlers keep task/request association fields populated.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind IN ('request_in_process','request_handled_by_task','request_parent')
GROUP BY kind;
```
