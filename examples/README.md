# Moire examples

Scenarios are implemented in `crates/moire-examples/src/scenarios`.

## Runner

Use the helper launcher to run moire-web (`--dev`) and one scenario in one command:

```bash
just ex channel-full-stall
```

When you stop the runner (`Ctrl+C`), all child processes are stopped too.

## 1) Oneshot sender lost in map (`tokio::sync::oneshot`)

Path: `crates/moire-examples/src/scenarios/oneshot_sender_lost_in_map.rs`

What it does:
- Creates a request/response oneshot pair
- Stores the sender in a pending map under the wrong key
- Receives a response but misses lookup, so the sender stays alive in the map and the receiver waits forever

### Run it

```bash
just ex oneshot-sender-lost-in-map
```

Then open [http://127.0.0.1:9131](http://127.0.0.1:9131) and inspect `demo.request_42.response`, `response_bus.recv`, and `request_42.await_response.blocked`.

## 2) Mutex lock-order inversion (`tokio` tasks + blocking mutex)

Path: `crates/moire-examples/src/scenarios/mutex_lock_order_inversion.rs`

What it does:
- Creates two shared mutexes (`demo.shared.left`, `demo.shared.right`)
- Starts two tracked Tokio tasks that intentionally acquire those mutexes in opposite order
- Uses a Tokio barrier so both tasks hold one lock before attempting the second lock, making the deadlock deterministic
- Exposes async symptoms with tracked observer tasks waiting forever on completion signals

### Run it

```bash
just ex mutex-lock-order-inversion
```

## 3) Channel full stall (`tokio::sync::mpsc` behavior)

Path: `crates/moire-examples/src/scenarios/channel_full_stall.rs`

What it does:
- Creates a bounded channel with capacity `16`
- Sends `16` items to fill it
- Attempts a 17th send, which blocks forever because the receiver is intentionally stalled

### Run it

```bash
just ex channel-full-stall
```

## 4) Roam RPC stuck request

Path: `crates/moire-examples/src/scenarios/roam_rpc_stuck_request.rs`

What it does:
- Starts an in-memory Roam client/server connection
- Sends one RPC request (`sleepy_forever`)
- Handler records a response node and then sleeps forever
- Caller remains blocked waiting for a response that never arrives

### Run it

Single-command runner:

```bash
just ex roam-rpc-stuck-request
```

## 5) Semaphore starvation (`tokio::sync::Semaphore`)

Path: `crates/moire-examples/src/scenarios/semaphore_starvation.rs`

What it does:
- Creates a semaphore with one permit
- `permit_holder` acquires and keeps that permit forever
- `permit_waiter` blocks forever trying to acquire a permit

### Run it

```bash
just ex semaphore-starvation
```

## 6) Roam Rust↔Swift stuck request

Path: `crates/moire-examples/src/scenarios/roam_rust_swift_stuck_request.rs`

What it does:
- Rust host starts a TCP listener and spawns a Swift roam-runtime peer (`swift run`)
- Rust accepts handshake and issues one raw RPC call
- Swift intentionally never answers that request, so Rust stays blocked waiting

### Run it

```bash
just ex roam-rust-swift-stuck-request
```

Requirements:
- Swift toolchain (`swift`) installed locally
- Local `../roam/swift/roam-runtime` checkout available
- Swift package files live in `crates/moire-examples/swift/roam-rust-swift-stuck-request`
