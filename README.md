# peeps

Production-ready instrumentation for Rust applications.

## Features

- 🔧 **Task Tracking**: Instrument all `tokio::spawn` calls with names, poll timing, and backtraces
- 🧵 **Thread Sampling**: Periodic SIGPROF-based sampling to detect stuck threads
- 🔒 **Lock Instrumentation**: Track parking_lot mutex/rwlock contention (TODO)
- 🌐 **Roam RPC Diagnostics**: Track connections, in-flight requests, control flow (TODO)
- 📊 **Interactive Dashboard**: Web UI to explore dumps and diagnose hangs
- 📝 **JSON Dumps**: Structured dumps to `/tmp/peeps-dumps/` on SIGUSR1

## Quick Start

```rust
use peeps::tasks;

#[tokio::main]
async fn main() {
    // Initialize instrumentation
    peeps::init();
    
    // Install SIGUSR1 handler for dumps
    peeps::install_sigusr1("my-service");
    
    // Use spawn_tracked instead of tokio::spawn
    tasks::spawn_tracked("my_task", async {
        // Your async code here
    });
    
    // Your application logic...
}
```

Trigger a dump with `kill -SIGUSR1 <pid>`, then view it:

```bash
cargo install peeps-dump
peeps  # Opens interactive dashboard
```

## Architecture

- `peeps-core`: Main API and dump collection
- `peeps-tasks`: Tokio task instrumentation
- `peeps-threads`: Thread sampling via SIGPROF
- `peeps-locks`: Lock contention tracking (TODO)
- `peeps-roam`: Roam RPC diagnostics (TODO)
- `peeps-dump`: CLI tool and dashboard server (TODO)
