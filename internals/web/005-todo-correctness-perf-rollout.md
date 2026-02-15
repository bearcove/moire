# Correctness, Perf, And Rollout Spec

Status: todo
Owner: wg-quality
Scope: tests, observability, migration plan

## Correctness tests

1. Ingest compatibility
- framed `ProcessDump` accepted from current producers
- malformed frame handling verified

2. Snapshot atomicity
- injected failure during write yields no partial rows

3. Projection invariants
- unique `(seq,id)` nodes
- unique `(seq,src_id,dst_id,kind)` edges
- required edge kinds present for representative dumps

4. Confidence invariants
- all causal edges have `confidence` in attrs

## Perf tests

- synthetic snapshots: 10k, 100k, 500k nodes
- benchmark:
  - latest snapshot query
  - stuck requests query
  - graph fetch/subgraph fetch

## Rollout phases

1. Run `peeps-web` alongside `peeps-dump`.
2. Validate parity for stuck request triage.
3. Move developers to `/investigate` flow.
4. Deprecate old dashboard after parity signoff.

## Signoff checklist

- [ ] Producers unchanged and compatible
- [ ] Snapshot pinning behavior verified
- [ ] Stuck-request workflow unblocks real bug triage
- [ ] No critical regressions vs old path for deadlock context
