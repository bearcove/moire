# Peeps Web Rebuild Overview

Status: todo
Owner: wg-peeps-web

## Objective

Build `peeps-web` as the canonical crate around a single graph model in SQLite:
- `nodes(seq, id, kind, process, attrs_json)`
- `edges(seq, src_id, dst_id, kind, attrs_json)`

No auto-refresh exploration. UI explores one snapshot (`seq`) at a time and only moves when user clicks "Jump to now".

## Non-negotiables

1. Canonical model is node/edge only (for causal analysis and graph exploration).
2. Frontend rebuilt from scratch (Vite + Preact), no legacy tab coupling.
3. Manual snapshot navigation (point-in-time exploration).
4. Single raw SQL HTTP endpoint for reads (`POST /api/sql`), so frontend and LLM workflows can iterate without backend query rewrites.
5. UI theme follows OS via CSS `light-dark()` (no manual toggle).

## Workstreams (parallelizable)

0. `000-todo-crate-split-for-parallelization.md`
1. `001-todo-storage-and-ingest.md`
2. `002-todo-node-edge-projection.md`
3. `003-todo-api-contract.md`
4. `004-todo-frontend-investigate-mvp.md`
5. `005-todo-correctness-local.md`
6. `006-todo-wrapper-emission-api.md`
7. `007-todo-resource-type-workstreams.md`

## Suggested execution order

- 000 runs first (enables safe parallel edits by resource).
- 001 + 002 + 003 can run in parallel.
- 004 starts once 003 endpoint stubs exist.
- 005 runs continuously as a local correctness checklist.

## Definition of done (program)

1. `peeps-web` receives JSON ingest frames from instrumented programs and persists snapshots.
2. SQLite stores canonical graph snapshots by `seq`.
3. UI starts from Requests and shows stuck RPCs (`>5s`) as a table.
4. UI does not auto-update selected snapshot.
5. "Jump to now" switches to latest seq snapshot explicitly.

## Why raw SQL

- Frontend can iterate quickly with HMR without backend endpoint churn.
- Snapshot persistence survives backend/frontend restarts.
- LLMs can issue SQL directly to investigate deadlocks and causal chains.

## Ingest format note (v1)

- Keep JSON ingest between internal programs for now.
- Wire format stays framed (`u32-be length` + JSON payload).
- Exact JSON schema must be pinned in `001-todo-storage-and-ingest.md` and versioned when changed.

## Initial product slice

1. Web UI stays stub-only while data model work lands.
2. Default query focus is stuck requests (`elapsed >= 5s`).
3. No ELK graph UI yet.
4. No kitchen-sink resource views ever.
