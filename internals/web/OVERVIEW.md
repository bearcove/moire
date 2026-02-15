# Peeps Web Rebuild Overview

Status: todo
Owner: wg-peeps-web

## Objective

Replace `peeps-dump` with a new crate `peeps-web` built around a single canonical graph model in SQLite:
- `nodes(seq, id, kind, process, attrs_json)`
- `edges(seq, src_id, dst_id, kind, attrs_json)`

No auto-refresh exploration. UI explores one snapshot (`seq`) at a time and only moves when user clicks "Jump to now".

## Non-negotiables

1. Canonical model is node/edge only (for causal analysis and graph exploration).
2. Frontend rebuilt from scratch (Vite + Preact), no legacy tab coupling.
3. Manual snapshot navigation (point-in-time exploration).
4. Ingest contract is explicitly defined by `peeps-web` requirements (not legacy protocol compatibility).

## Workstreams (parallelizable)

1. `001-todo-storage-and-ingest.md`
2. `002-todo-node-edge-projection.md`
3. `003-todo-api-contract.md`
4. `004-todo-frontend-investigate-mvp.md`
5. `005-todo-correctness-perf-rollout.md`

## Suggested execution order

- 001 + 002 + 003 can run in parallel.
- 004 starts once 003 endpoint stubs exist.
- 005 runs continuously and gates merge.

## Definition of done (program)

1. `peeps-web` receives instrumented event streams required to build node/edge snapshots.
2. SQLite stores canonical graph snapshots by `seq`.
3. UI shows stuck RPCs (`>5s`) and renders related graph for selected request.
4. UI does not auto-update selected snapshot.
5. "Jump to now" switches to latest seq snapshot explicitly.
