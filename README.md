# peeps

Low-overhead instrumentation for production Rust systems.

## Features

**Task Tracking**: Wraps `tokio::spawn` to record task names, poll timing, and execution backtraces. Each poll captures stack information when diagnostics are enabled.

**Thread Sampling**: SIGPROF-based periodic sampling identifies threads stuck in blocking operations or tight loops. Samples are aggregated to show dominant stack frames.

**Lock Instrumentation**: Tracks `parking_lot` mutex and rwlock contention (not yet implemented).

**Roam RPC Diagnostics**: Records connection state, in-flight requests, and control flow for applications using the roam RPC framework (not yet implemented).

**JSON Dumps**: On SIGUSR1, writes structured diagnostics to `/tmp/peeps-dumps/{pid}.json`. Format is stable and designed for programmatic consumption.

## Usage

```rust
use peeps::tasks;

#[tokio::main]
async fn main() {
    peeps::init();
    peeps::install_sigusr1("my-service");
    
    tasks::spawn_tracked("connection_handler", async {
        // Task execution is instrumented
    });
}
```

Trigger a dump:

```bash
kill -SIGUSR1 <pid>
```

View dumps:

```bash
cargo install peeps-dump
peeps
```

## Architecture

- `peeps`: Main API, dump collection, and SIGUSR1 integration
- `peeps-tasks`: Tokio task instrumentation
- `peeps-threads`: SIGPROF-based thread sampling
- `peeps-locks`: Lock contention tracking (planned)
- `peeps-roam`: Roam RPC diagnostics (planned)
- `peeps-dump`: CLI tool and dashboard server (planned)
