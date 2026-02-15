# Pre-007 Task: Split Resource Emitters Into Separate Files

Status: todo
Owner: wg-refactor
Priority: P0 (must complete before parallel resource tracks)

## Why this is required

Parallelizing resource work without file/module separation causes merge conflicts and hidden coupling.
We need one file/module per resource emitter before delegating track work.

## Scope

Split wrapper crates into resource-oriented modules and keep a thin facade `lib.rs`:

- `/Users/amos/bearcove/peeps/crates/peeps-tasks/src/`
  - `tasks.rs`
  - `futures.rs`
  - `wakes.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps-locks/src/`
  - `mutex.rs`
  - `rwlock.rs`
  - `registry.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps-sync/src/`
  - `mpsc.rs`
  - `oneshot.rs`
  - `watch.rs`
  - `semaphore.rs`
  - `oncecell.rs`
  - `registry.rs`
  - `snapshot.rs`
- `/Users/amos/bearcove/peeps/crates/peeps-threads/src/`
  - `sampling.rs`
  - `snapshot.rs`

## Constraints

1. No behavior changes in this task (refactor-only).
2. Public API of each crate remains source-compatible.
3. Add module-level tests where needed to preserve behavior.
4. Compile green after each crate split.

## Acceptance criteria

1. Each resource track in `007-*` can be edited mostly in isolated files.
2. Minimal cross-file touch needed for a given resource change.
3. `cargo check` passes workspace.
