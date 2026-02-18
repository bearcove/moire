# Peeps examples

These examples live in-repo but outside the workspace crates so they are easy to run and tweak.

## Runner

Use the helper launcher to run dashboard backend + frontend + any example in one command:

```bash
just ex --list
just ex channel-full-stall
```

When you stop the runner (`Ctrl+C`), all child processes are stopped too.

## 1) Oneshot sender lost in map (`tokio::sync::oneshot`)

Path: `examples/oneshot-sender-lost-in-map`

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

Path: `examples/mutex-lock-order-inversion`

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

Path: `examples/channel-full-stall`

What it does:
- Creates a bounded channel with capacity `16`
- Sends `16` items to fill it
- Attempts a 17th send, which blocks forever because the receiver is intentionally stalled

### Run it

Terminal 1:

```bash
cargo run -p peeps-web
```

Terminal 2:

```bash
pnpm --dir crates/peeps-web/frontend dev
```

Terminal 3:

```bash
PEEPS_DASHBOARD=127.0.0.1:9119 \
  cargo run --manifest-path examples/channel-full-stall/Cargo.toml
```

Then open [http://127.0.0.1:9131](http://127.0.0.1:9131) and inspect the `demo.work_queue` channel nodes plus the `queue.send.blocked` task.

## 4) Roam RPC stuck request

Path: `examples/roam-rpc-stuck-request`

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

Manual mode:

Terminal 1:

```bash
cargo run -p peeps-web
```

Terminal 2:

```bash
pnpm --dir crates/peeps-web/frontend dev
```

Terminal 3:

```bash
PEEPS_DASHBOARD=127.0.0.1:9119 \
  cargo run --manifest-path examples/roam-rpc-stuck-request/Cargo.toml
```

## 5) Semaphore starvation (`tokio::sync::Semaphore`)

Path: `examples/semaphore-starvation`

What it does:
- Creates a semaphore with one permit
- `permit_holder` acquires and keeps that permit forever
- `permit_waiter` blocks forever trying to acquire a permit

### Run it

```bash
just ex semaphore-starvation
```

## 6) Roam Rust↔Swift stuck request

Path: `examples/roam-rust-swift-stuck-request`

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
- Local `../roam/swift/roam-runtime` checkout available (used as a path dependency by the peer package)
