# Migration Spec: Consolidate peeps-* Crates into `peeps`

## Goal

Consolidate `peeps-futures`, `peeps-locks`, and `peeps-sync` into the top-level `peeps` crate so consumers can use:

- `use peeps::Mutex;`
- `use peeps::channel;`
- `use peeps::peep;`

while preserving feature-gated diagnostics and the canonical graph emission behavior.

**Additional requirement:** create a **single shared diagnostics registry** covering every tracked resource type (futures, locks, channels, oncecell, semaphores, RPC request/response, RPC tx/rx, etc.).

## Non-Goals

- Do not add new instrumentation features beyond current behavior.
- Do not reintroduce task tracking (task IDs, task snapshots, wake edges).
- Do not preserve `peeps-futures`, `peeps-locks`, or `peeps-sync` as separate published crates.

## Constraints

- Diagnostics must compile away to zero-cost stubs when the `diagnostics` feature is disabled.
- The spec is the source of truth; avoid ad-hoc manual workarounds.
- The new centralized registry must be the only authoritative registry for diagnostics.

---

## Target API Surface (examples)

Top-level exports from `peeps`:

- Locks
  - `Mutex`, `RwLock`
  - `AsyncMutex`, `AsyncRwLock`
- Sync primitives
  - `channel`, `unbounded_channel`, `oneshot_channel`, `watch_channel`
  - `Sender`, `Receiver`, `UnboundedSender`, `UnboundedReceiver`
  - `OneshotSender`, `OneshotReceiver`
  - `WatchSender`, `WatchReceiver`
  - `DiagnosticSemaphore`
  - `OnceCell`
- Futures
  - `spawn_tracked` (no-op wrapper now that tasks are removed)
  - `peep`, `peepable`, `peepable_with_meta`
  - `PeepableFuture`, `PeepableFutureExt`
- Graph collection
  - `collect_graph` uses the single registry to emit all resources

---

## Unified Registry Design

Create a new module `peeps::registry`:

### Registry Responsibilities

- Central storage of all live diagnostics objects, keyed as weak references.
- Snapshot extraction per resource type.
- Canonical graph emission per resource type.
- Shared process metadata (process name, proc_key).

### Registry Contents (minimum)

- Futures wait info, spawn/poll edges, wake/resume edges (task fields removed)
- Locks (mutex, rwlock, async locks)
- Channels:
  - mpsc
  - oneshot
  - watch
- Semaphores
- OnceCell
- RPC request/response and RPC channel endpoints (tx/rx)

### Registry Interface (sketch)

- `registry::init()` — no-op when diagnostics disabled
- `registry::set_process_info(process_name, proc_key)`
- `registry::snapshot_*()` (per resource type)
- `registry::emit_graph()` — emits canonical nodes/edges for all resources
- `registry::register_*()` (per resource type)
  - Example: `register_mpsc(info: &Arc<MpscInfo>)`
  - Example: `register_lock(info: &Arc<LockInfo>)`

All resource modules must register themselves into this registry, never maintain private registries.

---

## Migration Steps

### 1) Create `peeps::registry`
- New module under `peeps/src/registry.rs`.
- Aggregates registries currently living in `peeps-sync` and `peeps-locks`.
- Adds future-related registries from `peeps-futures`.
- Add RPC request/response and RPC channel endpoint tracking (hook into existing roam collection structures).

### 2) Move `peeps-futures` into `peeps`
- Create `peeps/src/futures/{mod.rs,enabled.rs,disabled.rs}` (shape similar to `peeps-locks`).
- Remove all task tracking references:
  - `TaskId`, `TaskSnapshot`, `TaskState`, `WakeEdgeSnapshot`, `task_name()`, `current_task_id()`.
- Update futures instrumentation to produce only future-related nodes/edges.
- Register all futures diagnostics in the **central registry**.

### 3) Move `peeps-locks` into `peeps`
- Create `peeps/src/locks/{mod.rs,enabled.rs,disabled.rs}`.
- Replace `peeps_futures::current_task_id()` usage with a removed/no-op or a placeholder (since tasks are removed).
- Register lock info in the **central registry** (no private lock registry).
- Preserve `DiagnosticMutex`, `DiagnosticRwLock`, `DiagnosticAsyncMutex`, `DiagnosticAsyncRwLock` behavior.

### 4) Move `peeps-sync` into `peeps`
- Create `peeps/src/sync/{mod.rs,channels.rs,semaphore.rs,oncecell.rs,enabled.rs,disabled.rs}`.
- Replace `crate::registry` usage with the new `peeps::registry`.
- Remove all `peeps_futures::task_name()` and `current_task_id()` references.

### 5) Update `collect_graph`
- Replace all calls to old crate-specific `emit_graph` functions.
- Use `registry::emit_graph(process_name, proc_key)` to emit all nodes/edges.
- Preserve roam graph emission but integrate its resources into the unified registry if appropriate.

### 6) Update `peeps` Public API
- Re-export types directly from the moved modules so `use peeps::Mutex;` works.
- Keep `peeps::futures` module for futures-specific helpers and macros, but also re-export macros at crate root.

### 7) Remove old crates
- Delete `crates/peeps-futures`, `crates/peeps-locks`, `crates/peeps-sync`.
- Remove dependencies from workspace and `peeps/Cargo.toml`.
- Update any references across the repo.

---

## Diagnostics Feature Flags

- Keep single `diagnostics` feature in `peeps`.
- Internally gate diagnostic codepaths (`#[cfg(feature = "diagnostics")]`).
- No cross-crate feature propagation since subcrates are removed.

---

## Verification Checklist

- `use peeps::Mutex` and `use peeps::channel` compile.
- All diagnostics compile away when `diagnostics` is disabled.
- `collect_graph` returns expected canonical nodes/edges.
- No references to removed task tracking remain.
- Registry is single source of truth for all resource tracking.

---

## Open Decisions (confirm with owner)

- Where to store RPC request/response state: registry or keep in roam session collection?
- Exact canonical node kinds for RPC tx/rx and request/response (use `NodeKind` contract).
- Whether to keep `peeps::futures::spawn_tracked` as a thin wrapper around `tokio::spawn` (recommended).
