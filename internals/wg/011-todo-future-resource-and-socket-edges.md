# Future->Resource And Socket Wait Edges

Status: todo
Owner: wg-resource-edges
Scope: `peeps-types`, sync/socket wrappers in Roam/Vixen, waitgraph ingestion

## Why

`future.resource` is currently a free-form string. This blocks strong joins to Locks/Channels views.

## Deliverables

## 1) Structured resource identity

```rust
pub enum ResourceRefSnapshot {
    Lock { process: String, name: String },
    Mpsc { process: String, name: String },
    Oneshot { process: String, name: String },
    Watch { process: String, name: String },
    Semaphore { process: String, name: String },
    OnceCell { process: String, name: String },
    RoamChannel { process: String, channel_id: u64 },
    Socket { process: String, fd: u64, label: Option<String> },
    Unknown { label: String },
}

pub struct FutureResourceEdgeSnapshot {
    pub future_id: FutureId,
    pub resource: ResourceRefSnapshot,
    pub wait_count: u64,
    pub total_wait_secs: f64,
    pub last_wait_age_secs: f64,
}
```

## 2) Socket wait details

For socket waits, include direction/reason:
- readable wait
- writable wait
- optionally peer endpoint if known

## 3) Stable resource URLs

All structured resources must map to stable dashboard routes so graph nodes are clickable.

## Acceptance Criteria

1. Graph node for a future waiting on lock/channel/socket links to corresponding resource page.
2. No string parsing required to identify resource kind.
3. Socket stalls can be surfaced in deadlock/causal analysis.
