# Correctness, Perf, And Rollout Spec

Status: todo
Owner: wg-quality
Scope: tests, observability, migration plan

## Correctness tests

1. Ingest contract
- framed payloads accepted per `peeps-web` ingest contract
- malformed frame handling verified

2. Snapshot atomicity
- injected failure during write yields no partial rows

3. Projection invariants
- unique `(seq,id)` nodes
- unique `(seq,src_id,dst_id,kind)` edges
- required edge kinds present for representative dumps

4. Explicit-edge invariants
- all stored causal edges are explicit instrumentation events (no heuristics)

## Perf tests

- synthetic snapshots: 10k, 100k, 500k nodes
- benchmark:
  - latest snapshot query
  - stuck requests query
  - graph fetch/subgraph fetch

## Rollout phases

1. Validate `peeps-web` stuck-request triage on real workloads.
2. Move developers to `/investigate` flow.
3. Iterate query packs and graph tooling for deadlock analysis.

## Signoff checklist

- [ ] Producers emit required explicit events
- [ ] Snapshot pinning behavior verified
- [ ] Stuck-request workflow unblocks real bug triage
- [ ] No critical regressions vs old path for deadlock context
