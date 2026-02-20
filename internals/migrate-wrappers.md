# Migrating moire wrappers to the new model

## What moire is

`moire` is a runtime observability layer for async Rust. It wraps tokio
primitives (channels, mutexes, semaphores, etc.) to track their state over
time. Every interesting state transition is recorded as a stream of changes,
which are:

1. Accumulated into snapshots
2. Sent to a server process
3. Persisted to SQLite
4. Queried and visualized in a browser

The goal is to make async programs inspectable after the fact: you can
reconstruct exactly what every task was doing at any point in time.

## Old model vs new model

### Old model

Each wrapper held a bundle of `Arc`-wrapped atomics and a shared
`Arc<StdMutex<SomeRuntimeState>>`. To record a state change, code would:

1. Lock the local runtime state (`self.channel.lock()`)
2. Mutate the fields
3. Call a sync function like `apply_channel_state(&self.channel)` which
   would lock the runtime DB and call a specific method like
   `db.update_channel_endpoint_state(...)`

This meant every state update touched two locks (local state + DB), and the
DB had bespoke methods for every kind of thing being tracked.

### New model

Everything goes through typed handles:

- **`EntityHandle<S>`** — owns a tracked entity; `S` is a slot type that
  constrains which entity body variant this handle may mutate
- **`WeakEntityHandle<S>`** — a non-owning reference to another entity;
  used when one wrapper needs to update a peer entity body without keeping
  it alive
- **`ScopeHandle`** — owns a tracked scope
- Event recording — via free functions (`record_event_with_source`)
- Edge recording — via `handle.link_to(...)` and similar

Both are defined in `crates/moire-runtime/src/handles.rs` and re-exported
from `moire_runtime`.

The local runtime state structs (`ChannelRuntimeState`, etc.) and all
`apply_*` / `sync_*` functions are **deleted entirely**.

## The four tracked kinds

**Entities** are long-lived runtime objects: channel endpoints, locks,
semaphores, futures, requests, etc. Their presence in the graph means they
are alive. When the owning `EntityHandle` is dropped, the entity is removed.

**Scopes** group entities (e.g., a task scope that owns all entities created
within a task).

**Edges** are directional relationships between entities:

```rust
// crates/moire-types/src/objects/edges.rs
pub enum EdgeKind {
    Polls,      // a future is actively polling this entity
    WaitingOn,  // a future is suspended waiting on this entity
    PairedWith, // two endpoints of the same channel (TX ↔ RX)
    Holds,      // a future/task holds a permit or lock on this entity
}
```

**Events** are point-in-time observations on an entity or scope.

## EntityHandle and mutate()

`EntityHandle<S>` is defined in `crates/moire-runtime/src/handles.rs`.
When the last clone drops, `HandleInner::drop` removes the entity from the
graph. **Entity presence = alive. Entity absent = dead.** No explicit close
cause fields are needed.

The `mutate` method (available when `S: EntityBodySlot`) is the only way
to update entity state:

```rust
// EntityHandle<S> where S: EntityBodySlot
pub fn mutate(&self, f: impl FnOnce(&mut S::Value)) -> bool
```

Under the hood `mutate_entity_body_and_maybe_upsert` hashes the body before
and after the closure. If the hash changed, it serializes the entity and
pushes a `UpsertEntity` change to the broadcast stream. **No change = no
broadcast.**

## WeakEntityHandle and cross-entity updates

When one wrapper needs to update a peer's entity body without keeping it
alive, use `WeakEntityHandle<S>` obtained via `handle.downgrade()`:

```rust
pub struct WeakEntityHandle<S = ()> {
    inner: Weak<HandleInner>,
    _slot: PhantomData<S>,
}
```

`WeakEntityHandle::mutate()` upgrades the `Weak` first. If the peer is
gone the upgrade fails and the call is silently a no-op — correct behavior
since there is nothing to update.

### MPSC queue length: canonical placement

Queue length lives on the **TX entity** (backpressure is the sender's
concern). The receiver holds a `WeakEntityHandle<MpscTxSlot>` and
decrements queue length on successful recv:

```rust
pub struct Sender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
    handle: EntityHandle<MpscTxSlot>,
}

pub struct Receiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
    handle: EntityHandle<MpscRxSlot>,
    tx_handle: WeakEntityHandle<MpscTxSlot>,
}
```

On send success:
```rust
self.handle.mutate(|body| body.queue_len += 1);
```

On recv success:
```rust
self.tx_handle.mutate(|body| body.queue_len -= 1); // no-op if sender gone
```

## Lifecycle via graph topology

**Do not track close causes or cross-side lifecycle events.**

- A receiver entity with no sender entity connected via `PairedWith` = dangling receiver
- No `tx_close_cause` / `rx_close_cause` fields
- No `ReceiverState` enum
- No `emit_channel_closed()` cross-side calls
- No custom `Drop` impls on channel/sync wrappers — `EntityHandle`'s Arc handles removal

`Clone` on a wrapper just clones its fields. No ref count bookkeeping.

## Entity body variants

These replace the old generic `ChannelTx(ChannelEndpointEntity)` /
`ChannelRx(ChannelEndpointEntity)` in `crates/moire-types/src/objects/entities.rs`.

### MPSC

```rust
// TX owns queue state
pub struct MpscTxEntity {
    pub queue_len: u32,
    pub capacity: Option<u32>,  // None = unbounded
}

// RX has no observable state beyond its existence in the graph
pub struct MpscRxEntity {}
```

### Broadcast

```rust
pub struct BroadcastTxEntity {
    pub capacity: u32,  // always bounded
}

// Each receiver tracks its own lag (messages dropped due to slow consumption)
pub struct BroadcastRxEntity {
    pub lag: u32,
}
```

### Watch

```rust
// TX records when the value was last written
pub struct WatchTxEntity {
    pub last_update_at: Option<PTime>,
}

pub struct WatchRxEntity {}
```

### Oneshot

```rust
// TX records whether the value has been sent
pub struct OneshotTxEntity {
    pub sent: bool,
}

pub struct OneshotRxEntity {}
```

The old `OneshotState` variants `SenderDropped` / `ReceiverDropped` are
gone — topology encodes those (missing TX entity = sender dropped, missing
RX entity = receiver dropped).

### Registration

Add these to the `define_entity_body!` invocation in
`crates/moire-types/src/objects/entities.rs` and remove the old
`ChannelTx` / `ChannelRx` variants:

```rust
crate::define_entity_body! {
    pub enum EntityBody {
        // ... existing variants ...
        MpscTx(MpscTxEntity),
        MpscRx(MpscRxEntity),
        BroadcastTx(BroadcastTxEntity),
        BroadcastRx(BroadcastRxEntity),
        WatchTx(WatchTxEntity),
        WatchRx(WatchRxEntity),
        OneshotTx(OneshotTxEntity),
        OneshotRx(OneshotRxEntity),
    }
}
```

## Events

### What changes

`ChannelWaitStarted` and `ChannelWaitEnded` are **removed**. The real-time
"blocked right now" signal is handled by edges (`WaitingOn`). Events are for
historical reconstruction only.

`ChannelClosed` is **removed**. Entity removal from the graph is the signal.

Wait duration is folded into the send/receive event as `wait_ns: Option<u64>`:
- `None` → operation returned immediately
- `Some(n)` → caller waited `n` nanoseconds before completing

### New event structs

```rust
pub struct ChannelSentEvent {
    pub wait_ns: Option<u64>,
    pub closed: bool,  // true if send failed (receiver gone)
}

pub struct ChannelReceivedEvent {
    pub wait_ns: Option<u64>,
    pub closed: bool,  // true if recv returned None (sender gone)
}
```

Remove from `EventKind`: `ChannelClosed`, `ChannelWaitStarted`, `ChannelWaitEnded`.

Remove structs: `ChannelClosedEvent`, `ChannelWaitStartedEvent`,
`ChannelWaitEndedEvent`, `ChannelWaitKind`.

Remove from `ChannelSendOutcome` / `ChannelReceiveOutcome`: the `Full` and
`Empty` variants (those were pre-wait markers; the `wait_ns` field replaces
them). The `Closed` variant is replaced by the `closed: bool` field.

## EdgeHandle — owned edges that clean up on drop

`EdgeHandle` is defined in `crates/moire-runtime/src/handles.rs`. It stores
the `(src, dst, kind)` triple of an edge and calls `db.remove_edge` when
dropped. It does **not** hold references to either entity — it only stores
`EntityId` values — so it does not keep entities alive. If an endpoint is
removed before the handle drops, the edge is already gone and Drop is a
no-op.

Create one via `EntityHandle::link_to_owned`:

```rust
let holds_edge: EdgeHandle = self.handle.link_to_owned(&holder_ref, EdgeKind::Holds);
```

This is the pattern for any edge that must be removed when a guard or permit
is released. The guard/permit struct holds the `EdgeHandle`; when it drops,
the edge is removed without any explicit unlink call.

### Mutex guard

```rust
pub struct MutexGuard<'a, T> {
    inner: parking_lot::MutexGuard<'a, T>,
    holds_edge: Option<EdgeHandle>,  // lock → holder; None if no causal target
}
```

Lock flow:
1. `try_lock` succeeds → `holds_edge = Some(handle.link_to_owned(&caller, Holds))`
2. Blocking `lock`:
   - Before: `waiting_edge = handle.link_to_owned(&caller, WaitingOn)`
   - After acquired: drop `waiting_edge`, set `holds_edge = Some(handle.link_to_owned(...))`
3. Guard drop → `holds_edge` drops → edge removed

### Semaphore permit

```rust
pub struct SemaphorePermit<'a> {
    inner: Option<tokio::sync::SemaphorePermit<'a>>,
    semaphore_handle: WeakEntityHandle<SemaphoreSlot>,
    holds_edge: Option<EdgeHandle>,  // semaphore → holder
    holder_counts: Arc<StdMutex<BTreeMap<EntityRef, u32>>>,
}
```

`holder_counts` stays as internal bookkeeping to know when to drop the
`holds_edge` (a future may hold multiple permits; only remove the edge when
the count reaches zero).

## Never touch the database directly

Wrapper code in `crates/moire/src/enabled/` must never call `runtime_db()`
or acquire the DB lock directly. All state changes go through the handle
API:

- **Entity state** → `handle.mutate(|body| ...)` or `weak_handle.mutate(...)`
- **Edges** → `handle.link_to(...)`, `handle.link_to_handle(...)`
- **Scope links** → `handle.link_to_scope(...)`, `handle.unlink_from_scope(...)`
- **Events** → `record_event_with_source(event, &source)`

If you find yourself reaching for `runtime_db().lock()` in a wrapper, that
is a sign the operation should be expressed through one of the above instead.

## Source

Sources used to be a raw `String` (`"file.rs:42"`) plus a separate `krate`
field. They are now a unified `Source` type from `crates/moire-source`,
interned as `SourceId`.

Constructors for entities, scopes, and edges all accept `impl Into<SourceId>`.
Wrapper methods receive `Source` from the `facade!`-generated extension
traits and pass it through unchanged.

## Method naming rules

These rules apply to wrapper types in `crates/moire/src/enabled/`:

| Rule | Detail |
|---|---|
| No `#[track_caller]` on wrapper impl methods | Only `facade!`-generated extension trait methods are `#[track_caller]` |
| Use `async fn` directly | No `#[allow(clippy::manual_async_fn)]`, no `-> impl Future<...>` workarounds |
| Remove all `_with_cx` methods | Gone entirely; `facade!` handles source joining |
| Mark `_with_source` methods `#[doc(hidden)]` | Public API is the extension traits |
| No interior mutable state beyond handles | No `Arc<StdMutex<...>>` fields; all state lives in entity bodies |
| No `name: String` field | Name is already in the entity |
| No `channel: Arc<StdMutex<...>>` field | Deleted; was the old local runtime state |

### Edge creation: synchronous operations

`try_send` and other non-suspending operations create a `Polls` edge from
the current causal future to the target entity:

```rust
pub fn try_send(&self, value: T, source: Source) -> Result<...> {
    if let Some(caller) = current_causal_target() {
        self.handle.link_to(&caller, EdgeKind::Polls);
    }
    // ...
}
```

`current_causal_target()` is in `crates/moire-runtime/src/handles.rs` and
reads from the `FUTURE_CAUSAL_STACK` task-local.

### Edge creation: suspending operations

Async send/recv go through `instrument_operation_on_with_source`, which
creates a `Polls` edge that upgrades to `WaitingOn` if the future suspends.

### TX ↔ RX pairing

At channel creation, link the two endpoints:

```rust
tx_handle.link_to_handle(&rx_handle, EdgeKind::PairedWith);
```

### Mutex and RwLock

Use `EdgeKind::Polls` for the lock attempt edge, `EdgeKind::WaitingOn` if
the future suspends on the lock, `EdgeKind::Holds` while the guard is held.
Rename to more precise variants later if needed.

## Per-wrapper migration reference

### channels/mpsc.rs — `Sender<T>`, `Receiver<T>`, `UnboundedSender<T>`, `UnboundedReceiver<T>`

**Delete:** `ChannelRuntimeState`, `ReceiverState`, `channel: Arc<StdMutex<...>>` field, `name: String` field.

**Struct layout after:**
```rust
pub struct Sender<T> {
    inner: tokio::sync::mpsc::Sender<T>,
    handle: EntityHandle<MpscTxSlot>,
}
pub struct Receiver<T> {
    inner: tokio::sync::mpsc::Receiver<T>,
    handle: EntityHandle<MpscRxSlot>,
    tx_handle: WeakEntityHandle<MpscTxSlot>,
}
// UnboundedSender / UnboundedReceiver: same pattern
```

**Key operations:**
- Send success: `self.handle.mutate(|body| body.queue_len += 1)`
- Recv success: `self.tx_handle.mutate(|body| body.queue_len -= 1)` (no-op if sender gone)
- No custom `Drop` impls — `EntityHandle` Arc handles removal
- `Clone` on `Sender`: trivial, just clone both fields

---

### channels/broadcast.rs — `BroadcastSender<T>`, `BroadcastReceiver<T>`

**Delete:** `BroadcastRuntimeState`, `receiver_handle: EntityHandle` from sender, `channel: Arc<StdMutex<...>>` field.

**Struct layout after:**
```rust
pub struct BroadcastSender<T> {
    inner: tokio::sync::broadcast::Sender<T>,
    handle: EntityHandle<BroadcastTxSlot>,
}
pub struct BroadcastReceiver<T> {
    inner: tokio::sync::broadcast::Receiver<T>,
    handle: EntityHandle<BroadcastRxSlot>,
    tx_handle: WeakEntityHandle<BroadcastTxSlot>,
}
```

**Key operations:**
- Send: emit `ChannelSentEvent` (with `wait_ns: None`, broadcasts don't block)
- Recv success: update `lag` on own entity via `self.handle.mutate(|body| body.lag = ...)`
  — tokio's `Receiver::len()` gives the current lag
- Recv with lagged error: same, update lag
- `Clone` on sender/receiver: trivial

---

### channels/watch.rs — `WatchSender<T>`, `WatchReceiver<T>`

**Delete:** `WatchRuntimeState`, `receiver_handle`, `channel: Arc<StdMutex<...>>` field.

**Struct layout after:**
```rust
pub struct WatchSender<T> {
    inner: tokio::sync::watch::Sender<T>,
    handle: EntityHandle<WatchTxSlot>,
}
pub struct WatchReceiver<T> {
    inner: tokio::sync::watch::Receiver<T>,
    handle: EntityHandle<WatchRxSlot>,
    tx_handle: WeakEntityHandle<WatchTxSlot>,
}
```

**Key operations:**
- Send/`send_modify`: `self.handle.mutate(|body| body.last_update_at = Some(PTime::now()))`
- Changed wait: goes through `instrument_operation_on_with_source`
- `Clone` on receiver: trivial; note that tokio's `watch::Receiver::clone()` subscribes a new receiver

---

### channels/oneshot.rs — `OneshotSender<T>`, `OneshotReceiver<T>`

**Delete:** `OneshotRuntimeState`, `channel: Arc<StdMutex<...>>` field.

**Struct layout after:**
```rust
pub struct OneshotSender<T> {
    inner: tokio::sync::oneshot::Sender<T>,
    handle: EntityHandle<OneshotTxSlot>,
}
pub struct OneshotReceiver<T> {
    inner: tokio::sync::oneshot::Receiver<T>,
    handle: EntityHandle<OneshotRxSlot>,
    tx_handle: WeakEntityHandle<OneshotTxSlot>,
}
```

**Key operations:**
- `send()` success: `self.handle.mutate(|body| body.sent = true)` before handle drops
- Recv: `instrument_operation_on_with_source`, emit `ChannelReceivedEvent`
- No `Clone` — oneshot endpoints are non-cloneable

---

### sync/mutex.rs — `Mutex<T>`, `MutexGuard<'a, T>`

**Delete:** all direct `runtime_db()` calls in `wrap_guard`, `record_pending_wait_edges`,
`clear_pending_wait_edges`, and `MutexGuard::drop`. The `lock_with_cx` method.

**Struct layout after:**
```rust
pub struct Mutex<T> {
    inner: parking_lot::Mutex<T>,
    handle: EntityHandle<LockSlot>,
}
pub struct MutexGuard<'a, T> {
    inner: parking_lot::MutexGuard<'a, T>,
    lock_handle: WeakEntityHandle<LockSlot>,  // to manage Holds edge on drop
    owner_ref: Option<EntityRef>,              // causal future that holds the lock
}
```

**Edge pattern:** use `EdgeHandle` (see the EdgeHandle section above).

- `try_lock` success: `holds_edge = Some(handle.link_to_owned(&caller, Holds))`
- Blocking `lock`: create `waiting_edge` before, drop it after acquired, then create `holds_edge`
- Guard drop: `holds_edge` drops automatically

`HELD_MUTEX_STACK` management stays as-is (it's task-local, not entity state).

---

### sync/rwlock.rs — `RwLock<T>`

Currently returns parking_lot guards directly with no edge tracking. Source parameter
is silently ignored. The `_with_cx` methods need removal.

**Struct layout after:** unchanged (`inner` + `handle: EntityHandle<LockSlot>`).

**What changes:**
- Remove `read_with_cx`, `write_with_cx`, `try_read_with_cx`, `try_write_with_cx`
- Make `read_with_source`, `write_with_source`, etc. `#[doc(hidden)]`
- Add `Polls` edge from `current_causal_target()` to the lock on each read/write attempt
  (wrapping parking_lot guards to also remove the edge on drop is a stretch goal)

---

### sync/notify.rs — `Notify`

**Delete:** `waiter_count: Arc<AtomicU32>` field, direct `runtime_db()` calls,
`notified_with_cx`.

**Struct layout after:**
```rust
pub struct Notify {
    inner: Arc<tokio::sync::Notify>,
    handle: EntityHandle<NotifySlot>,
}
```

**Key operations:**
- Before `notified()` wait: `self.handle.mutate(|body| body.waiter_count += 1)`
- After `notified()` returns: `self.handle.mutate(|body| body.waiter_count -= 1)`
- `notify_one` / `notify_waiters`: no entity state change needed

---

### sync/once_cell.rs — `OnceCell<T>`

**Delete:** `waiter_count: AtomicU32` field, all direct `runtime_db()` calls,
`get_or_init_with_cx`, `get_or_try_init_with_cx`.

**Struct layout after:**
```rust
pub struct OnceCell<T> {
    inner: tokio::sync::OnceCell<T>,
    handle: EntityHandle<OnceCellSlot>,
}
```

**Key operations:**
- Before init wait: `self.handle.mutate(|body| { body.waiter_count += 1; body.state = Initializing; })`
- After init completes: `self.handle.mutate(|body| { body.waiter_count -= 1; body.state = ...; })`
- `set()`: `self.handle.mutate(|body| body.state = if initialized { Initialized } else { ... })`

---

### sync/semaphore.rs — `Semaphore`, `SemaphorePermit<'a>`, `OwnedSemaphorePermit`

**Delete:** `max_permits: Arc<AtomicU32>`, `holder_counts: Arc<StdMutex<BTreeMap<...>>>`,
`sync_semaphore_state`, `release_semaphore_holder_edge`, all direct `runtime_db()` calls,
all `_with_cx` methods.

The `holder_counts` map (tracking per-future permit counts to know when to remove the
`Holds` edge) is internal bookkeeping that does not constitute entity state. It can stay
as a local `Arc<StdMutex<...>>` shared between the semaphore and its permits — this does
not violate the no-DB-direct rule since it is not a DB write path.

**Struct layout after:**
```rust
pub struct Semaphore {
    inner: Arc<tokio::sync::Semaphore>,
    handle: EntityHandle<SemaphoreSlot>,
    max_permits: u32,  // plain field, updated via handle.mutate on add_permits
    holder_counts: Arc<StdMutex<BTreeMap<EntityRef, u32>>>,
}
pub struct SemaphorePermit<'a> {
    inner: Option<tokio::sync::SemaphorePermit<'a>>,
    semaphore_handle: WeakEntityHandle<SemaphoreSlot>,
    holder_ref: Option<EntityRef>,
    holder_counts: Arc<StdMutex<BTreeMap<EntityRef, u32>>>,
}
// OwnedSemaphorePermit: same, without the lifetime
```

**Key operations:**
- After acquire: `self.handle.mutate(|body| body.handed_out_permits = max - available)`
- Holder edge: `holds_edge = Some(self.handle.link_to_owned(&holder_ref, EdgeKind::Holds))`
  stored on the permit struct — drops automatically when permit drops
- Permit drop: `semaphore_handle.mutate(...)` for state update; `holds_edge` removes itself
- `add_permits`: `self.handle.mutate(|body| { body.max_permits += n; body.handed_out_permits = ...; })`

---

### joinset.rs — `JoinSet<T>`

Very close to correct already — no direct DB calls, no interior mutable state issues.

**What changes:**
- Remove `spawn_with_cx`, `join_next_with_cx`
- Make `spawn_with_source`, `join_next_with_source` `#[doc(hidden)]`
- `EntityBody::Future` is fine as the entity body for now

---

### rpc.rs — `RpcRequestHandle`, `RpcResponseHandle`

**Delete:** all direct `runtime_db()` calls. `EdgeKind::RpcLink` does not exist in the
current EdgeKind set — use `EdgeKind::PairedWith` to link request → response.

**`set_status`** — currently calls `runtime_db()` twice. After:
```rust
pub fn set_status(&self, status: ResponseStatus, source: Source) {
    self.handle.mutate(|body| body.status = status);
    // emit StateChanged event via record_event_with_source
}
```
Note: `set_status` needs a `Source` parameter added, or the caller methods
(`mark_ok`, `mark_error`, `mark_cancelled`) can capture it via `SourceRight::caller()`
and pass it through.

**`rpc_response_for`** — currently reads the request entity's source from the DB and
stamps it on the response. This is wrong: requests and responses can be in different
processes, and the response source must be where the handler runs, not where the request
originated. The `source: SourceRight` parameter is already correct. Delete the entire
DB lookup. The function becomes:

```rust
pub fn rpc_response_for(
    method: impl Into<String>,
    request: &EntityRef,
    source: SourceRight,
) -> RpcResponseHandle {
    let method = method.into();
    let body = EntityBody::Response(ResponseEntity {
        method: method.clone(),
        status: ResponseStatus::Pending,
    });
    let response = RpcResponseHandle {
        handle: EntityHandle::new(method, body, source),
    };
    response.handle.link_to(request, EdgeKind::PairedWith);
    response
}
```

## What is deleted

After migrating a wrapper, the following are deleted:

- The `*RuntimeState` struct (e.g., `ChannelRuntimeState`, `BroadcastRuntimeState`, `OneshotRuntimeState`, `WatchRuntimeState`)
- Lifecycle enums local to the module (`ReceiverState`, etc.)
- All `apply_*` functions (`apply_channel_state`, `apply_broadcast_state`, `apply_oneshot_state`, `apply_watch_state`)
- All `sync_*` functions (`sync_channel_state`, etc.)
- All `_with_cx` methods on wrapper types
- The `channel: Arc<StdMutex<...>>` field on wrapper structs
- The `name: String` field on wrapper structs
- `#[allow(clippy::manual_async_fn)]` attributes
- `#[track_caller]` attributes on wrapper impl methods
- The corresponding `db.update_*` methods in `crates/moire-runtime/src/db.rs` that are no longer called
- `ChannelClosedEvent`, `ChannelWaitStartedEvent`, `ChannelWaitEndedEvent`, `ChannelWaitKind`
- `ChannelEndpointLifecycle`, `ChannelCloseCause`, `ChannelDetails`, `*ChannelDetails`
- Old `ChannelTx` / `ChannelRx` entity body variants and `ChannelEndpointEntity`
