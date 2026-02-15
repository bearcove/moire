# Edge Confidence And Temporal Model

Status: done
Owner: wg-analysis
Scope: `peeps-waitgraph`, deadlock detector, dashboard graph renderers

## Why

Today explicit and inferred edges are mixed without confidence semantics. Aggregate-only edges also hide ordering.

## Deliverables

## 1) Confidence model

Add confidence to graph edges:
- `explicit`: emitted directly by instrumentation
- `derived`: inferred from snapshots
- `heuristic`: guess-level correlation

## 2) Temporal model

For each edge, provide:
- last_seen_age_secs
- optional first_seen_age_secs
- optional sample_count

This supports "stale edge" filtering.

## 3) UI semantics

- solid lines: explicit
- dashed lines: derived
- dotted lines: heuristic
- graph toggles: `all`, `explicit-only`

## 4) Deadlock scoring adjustments

Reduce score when cycle depends mostly on non-explicit edges.

## Acceptance Criteria

1. Every rendered edge reports confidence.
2. User can hide non-explicit edges in request graph and deadlock views.
3. Cycle rationale text includes confidence breakdown.
