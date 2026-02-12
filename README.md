# peeps

Low-overhead instrumentation for production Rust systems.

## Features

**Task Tracking**: Wraps `tokio::spawn` to record task names, poll timing, and execution backtraces. Each poll captures stack information when diagnostics are enabled.

**Thread Sampling**: SIGPROF-based periodic sampling identifies threads stuck in blocking operations or tight loops. Samples are aggregated to show dominant stack frames.

**Lock Instrumentation**: Tracks `parking_lot` mutex/rwlock contention when the `locks` feature is enabled.

**Roam + SHM Diagnostics**: Records connection state and shared-memory diagnostics from registered providers.

**JSON Dumps**: On SIGUSR1, writes structured diagnostics to `/tmp/peeps-dumps/{pid}.json`. Format is stable and designed for programmatic consumption.

**Live Dashboard**: A `peeps` CLI server receives push snapshots and serves a web UI.

## Usage

### 1) File-based dumps (SIGUSR1)

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

Open dashboard from dump files:

```bash
cargo install peeps-dump
peeps
```

The dashboard server reads existing JSON dumps from `/tmp/peeps-dumps` on startup.

### 2) Live dashboard push mode

Start the dashboard server:

```bash
cargo install peeps-dump
peeps
```

By default:
- TCP ingest: `127.0.0.1:9119` (`PEEPS_LISTEN`)
- HTTP UI: `127.0.0.1:9120` (`PEEPS_HTTP`)

Enable dashboard push in your app and call `init_named`:

```toml
peeps = { git = "https://github.com/bearcove/peeps", branch = "main", features = ["dashboard"] }
```

```rust
#[tokio::main]
async fn main() {
    peeps::init_named("my-service");
}
```

Then run your app with:

```bash
PEEPS_DASHBOARD=127.0.0.1:9119 <your-binary>
```

## Architecture

- `peeps`: Main API, dump collection, SIGUSR1 integration, optional dashboard push client
- `peeps-tasks`: Tokio task instrumentation
- `peeps-threads`: SIGPROF-based thread sampling
- `peeps-locks`: Lock contention tracking
- `peeps-dump`: CLI tool and dashboard server (binary name: `peeps`)
