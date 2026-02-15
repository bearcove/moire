# Resource Track: Futures

Status: todo
Owner: wg-resource-futures
Priority: P0

## Mission

Represent instrumented futures as first-class nodes with explicit `needs` dependencies.

## Prerequisites

- Complete `/Users/amos/bearcove/peeps/internals/web/000-todo-crate-split-for-parallelization.md`.
- Use contracts from `/Users/amos/bearcove/peeps/internals/web/006-todo-wrapper-emission-api.md`.

## Current context

- Future instrumentation is in `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/futures.rs` and `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/snapshot.rs`.
- Current `peepable` API is label-only; metadata-capable API must be added.

## API contract (concrete)

Required public API (v1):

```rust
pub enum MetaValue<'a> {
    Static(&'static str),
    Str(&'a str),
    U64(u64),
    I64(i64),
    Bool(bool),
}

pub struct MetaBuilder<'a, const N: usize> { /* stack storage */ }
impl<'a, const N: usize> MetaBuilder<'a, N> {
    pub fn push(&mut self, key: &'static str, value: MetaValue<'a>);
}

pub fn peepable<F>(
    future: F,
    label: &'static str,
) -> PeepableFuture<F>;

pub fn peepable_with_meta<F, const N: usize>(
    future: F,
    label: &'static str,
    meta: MetaBuilder<'_, N>,
) -> PeepableFuture<F>;

pub trait PeepableFutureExt: Future + Sized {
    fn peepable(self, label: &'static str) -> PeepableFuture<Self>;
    fn peepable_with_meta<const N: usize>(
        self,
        label: &'static str,
        meta: MetaBuilder<'_, N>,
    ) -> PeepableFuture<Self>;
}

#[macro_export]
macro_rules! peep_meta {
    ($($k:literal => $v:expr),* $(,)?) => { /* builds MetaBuilder on stack */ };
}

#[macro_export]
macro_rules! peepable_with_meta {
    ($future:expr, $label:literal, {$($k:literal => $v:expr),* $(,)?}) => {{
        #[cfg(feature = "diagnostics")]
        { $crate::PeepableFutureExt::peepable_with_meta($future, $label, $crate::peep_meta!($($k => $v),*)) }
        #[cfg(not(feature = "diagnostics"))]
        { $future }
    }};
}
```

Semantics:
- `peepable(label)` is exactly equivalent to `peepable_with_meta(label, empty_metadata)`.
- metadata is attached to the future node attrs (`attrs_json.meta`) at creation time.
- metadata updates are out of scope for v1 (immutable per wrapped future).
- when diagnostics feature is disabled, metadata expressions must not be evaluated.
- hot-path default must be zero heap allocations.

Metadata key rules:
- ASCII keys only: `[a-z0-9_.-]+`
- max key length: 48 bytes
- max value length: 256 bytes
- max metadata pairs per future: 16
- invalid entries are dropped (do not panic)

Recommended canonical keys:
- `request.id`
- `request.method`
- `request.correlation_key`
- `rpc.connection`
- `channel.id`
- `resource.path`

## Node + edge model

Node ID:
- `future:{proc_key}:{future_id}`

Node kind:
- `future`

Required attrs_json:
- `future_id`
- `label`
- `pending_count`
- `ready_count`
- `meta` (shared metadata object, arbitrary key/value metadata)

Optional attrs_json:
- `created_by_task_id`
- `last_polled_by_task_id`
- `total_pending_ns`

Required `needs` edges:
- `task -> future` (from explicit poll/wait records)
- `future -> resource` only when explicitly measured

Optional `needs` edges:
- `future -> task` only when explicitly measured as dependency

## Implementation steps

1. Add metadata-capable API:
- `peepable_with_meta(future, label, metadata)`
- keep `peepable(label)` as convenience wrapper.
2. Persist metadata on future node attrs.
3. Emit only explicitly recorded `needs` dependencies.
4. Do not invent edge semantics beyond `needs`.
5. Ensure `Future + Send + 'static` ergonomics remain unchanged for existing `peepable(...)` callsites.
6. Add unit tests for:
- metadata acceptance/clamping/dropping invalid keys
- equivalence of `peepable(...)` and `peepable_with_meta(..., empty)`
- serialization shape of `attrs_json.meta`

## Consumer changes

Required:
- Add `peepable_with_meta` at important await points in Roam/Vixen:
  - request_id
  - method
  - channel_id
  - path/resource key

Migration pattern:

Before:
```rust
stream.read(&mut buf).peepable("socket.read").await?;
```

After:
```rust
peeps_tasks::peepable_with_meta!(
    stream.read(&mut buf),
    "socket.read",
    {
        "request.id" => request_id,
        "request.method" => method_name,
        "rpc.connection" => conn_id,
    }
)
.await?;
```

## Validation SQL

```sql
SELECT COUNT(*)
FROM nodes
WHERE snapshot_id = ?1 AND kind = 'future';
```

```sql
SELECT COUNT(*)
FROM edges
WHERE snapshot_id = ?1
  AND kind = 'needs'
  AND (
    (src_id LIKE 'task:%' AND dst_id LIKE 'future:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'lock:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'mpsc:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'oneshot:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'watch:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'semaphore:%')
    OR (src_id LIKE 'future:%' AND dst_id LIKE 'oncecell:%')
  );
```
