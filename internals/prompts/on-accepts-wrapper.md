# Prompt: Make `on =` accept wrapped channel types directly

## Background

The `peeps!()` macro accepts an `on =` parameter that links an instrumented future to a
channel endpoint, creating `Polls`/`Needs` edges in the graph. Currently this requires
calling `.handle()` on the wrapper type:

```rust
let rx_handle = rx.handle().clone();
peeps!(name = "my_future", on = rx_handle, fut = some_async_work()).await;
```

This is bad because `.handle()` is a peeps-specific method that real programs would never
call. The goal is to make `on =` accept the wrapped type directly:

```rust
peeps!(name = "my_future", on = rx, fut = some_async_work()).await;
```

## How `on =` works today

The macro expands `on = $on` to `&$on` and passes it to:

```rust
pub fn instrument_future_on_with_krate<F>(
    name: impl Into<String>,
    on: &EntityHandle,   // ← currently hardcoded to &EntityHandle
    fut: F,
    source: impl Into<String>,
    krate: impl Into<String>,
) -> InstrumentedFuture<F>
```

Inside, it calls `on.entity_ref()` to get an `EntityRef` (just a lightweight ID wrapper).
That `EntityRef` is stored in `InstrumentedFuture` and used during polling to emit
`Polls`/`Needs` edges.

All wrapped receiver/sender types (e.g. `Receiver<T>`, `BroadcastReceiver<T>`,
`WatchReceiver<T>`, `OneshotReceiver<T>`, `UnboundedReceiver<T>`, and their TX
counterparts) hold `handle: EntityHandle` and expose it via `.handle() -> &EntityHandle`.

## What needs to change

1. **Add a trait** (e.g. `HasEntityHandle` or `AsEntityRef`) in the enabled/public API:

   ```rust
   pub trait AsEntityRef {
       fn as_entity_ref(&self) -> EntityRef;
   }
   ```

2. **Implement it for `EntityHandle`** (and its `Clone` since `rx.handle().clone()` returns
   an `EntityHandle`):

   ```rust
   impl AsEntityRef for EntityHandle {
       fn as_entity_ref(&self) -> EntityRef { self.entity_ref() }
   }
   ```

3. **Implement it for every wrapped channel type** — `Receiver<T>`, `Sender<T>`,
   `BroadcastReceiver<T>`, etc. — by delegating to their inner `handle`:

   ```rust
   impl<T> AsEntityRef for Receiver<T> {
       fn as_entity_ref(&self) -> EntityRef { self.handle.entity_ref() }
   }
   ```

4. **Update `instrument_future_on_with_krate`** to accept `&impl AsEntityRef` instead of
   `&EntityHandle`:

   ```rust
   pub fn instrument_future_on_with_krate<F>(
       name: impl Into<String>,
       on: &impl AsEntityRef,   // ← accepts any wrapped type
       ...
   )
   ```

5. **Mark `handle()` as `#[doc(hidden)]`** on all wrapped types. It can stay pub for now
   as an escape hatch, but should not appear in docs.

6. **Update the example** `examples/channel-full-stall/src/main.rs` to use `on = rx`
   (not `on = rx.handle().clone()`). The example should not call `.handle()`.

## Disabled/stub path

When peeps is disabled (stub mode), `instrument_future_on_with_krate` is a no-op and
the trait just needs a stub impl. Make sure the trait is available in both paths or
gated appropriately so the call sites compile either way.

## What not to change

- The `peeps!()` macro expansion itself (`&$on`) does not need to change — `&rx`
  satisfies `&impl AsEntityRef` once the impls exist.
- `InstrumentedFuture`, `FutureEdgeRelation`, `EntityRef` internals don't need to change.
- Do not remove `handle()` — just `#[doc(hidden)]` it.
