# Request-Task Binding And Cross-Process Request Edges

Status: todo
Owner: wg-rpc-context
Scope: Roam/Vixen request instrumentation + `peeps-types` + dashboard readers

## Why

Request trees currently rely on span metadata heuristics. We need strict identity links.

## Deliverables

## 1) Strict request->server-task binding

For each in-flight request, emit handling task identity explicitly:
- `server_task_id`
- `server_task_name`
- `server_process`

## 2) Strict cross-process request parent edge

Emit explicit parent references:

```rust
pub struct RequestParentRef {
    pub parent_process: String,
    pub parent_connection: String,
    pub parent_request_id: u64,
}
```

Attach to request snapshots where available.

## 3) Context propagation contract

Define reserved metadata keys and transport behavior:
- stable chain/span identifiers
- parent request ref propagation on outgoing calls
- handoff to server-side task at receipt

## Acceptance Criteria

1. Request tree reconstruction works even if span names are missing.
2. Cross-process parent-child requests can be reconstructed from explicit fields only.
3. Request card can always link to owning task when server-side context exists.
