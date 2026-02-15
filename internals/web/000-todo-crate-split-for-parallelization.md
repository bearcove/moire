# Pre-007 Task: Split Wrappers For Parallel Work

Status: todo
Owner: wg-refactor
Priority: P0

## Goal

Restructure wrapper crates so resource tracks can be implemented in parallel with minimal merge conflicts.

## Scope

Refactor-only (no behavior change) into resource-focused modules with thin `lib.rs` facades:

- `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/`
  - `tasks.rs`
  - `futures.rs`
  - `wakes.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps-locks/src/`
  - `sync_locks.rs`
  - `registry.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps-sync/src/`
  - `channels.rs` (mpsc/oneshot/watch endpoint model)
  - `semaphore.rs`
  - `oncecell.rs`
  - `registry.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps/src/`
  - `collect.rs` (graph assembly)
  - `dashboard_client.rs`

## Constraints

1. No feature/API behavior changes in this task.
2. Public crate APIs remain source-compatible.
3. Keep tests green and add module-level tests where extraction needs coverage.

Policy note:
- Async mutex/rwlock wrappers are out of scope (banned).
- `OnceCell` remains in scope under `peeps-sync`.

## Acceptance criteria

1. Each active 007 track can be edited mostly in isolated files.
2. Workspace `cargo check` remains green.
