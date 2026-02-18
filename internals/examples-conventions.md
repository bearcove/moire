# Example Conventions

This is the contract for examples in this repo.

The goal is simple: `just ex` should be enough to reproduce a scenario, inspect it, and stop cleanly without leaving stray processes behind.

## User Flow

The intended loop is:

1. Run `just ex`.
2. Pick an example from the selector.
3. Watch it boot everything required.
4. Inspect in the UI.
5. Stop with `Ctrl+C`.

No second terminal should be required for normal use.

## Runtime Contract

Every example must satisfy this:

- `cargo run --manifest-path examples/<name>/Cargo.toml` runs the full scenario.
- If the scenario needs multiple roles (client/server, caller/callee, etc.), the example binary itself is responsible for launching and coordinating them.
- Example startup should fail fast with a clear error if one required role cannot start.
- Example shutdown should terminate all child work it started.

The runner may set environment (`PEEPS_DASHBOARD`, ports), but it should not contain scenario-specific orchestration logic.

## Process-Group Contract

`peeps-examples` (`cargo run --bin peeps-examples`, used by `just ex`) is responsible for top-level lifecycle:

- It starts `peeps-web` in one process group.
- It starts the chosen example (`cargo run`) in another process group.
- On exit or interrupt, it sends a group kill to both groups and waits for them.

This is required to avoid zombie/orphaned children when examples spawn subprocesses.

## Authoring Rules For Multi-Process Examples

When an example needs multiple processes:

- Keep orchestration inside the example crate.
- Prefer explicit supervised child handles over detached subprocesses.
- Propagate cancellation and wait for child exit paths.
- Treat partial startup as an error; tear down anything already started.

If a scenario cannot be expressed as a single `cargo run` entrypoint, it does not meet this repo's examples contract yet.
