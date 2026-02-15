# Causality Gaps Program Overview

Status: todo
Owner: wg-causality-program

## Goal

Make request-level causal graphs and deadlock detection trustworthy by replacing inferred links with explicit edges.

## Problem Summary

Current dashboards mix explicit and inferred relationships without clear distinction. This leads to:
- ambiguous edges (`touches`-style associations)
- false-positive cycles
- inability to answer "what exactly is waiting on what" for cross-process RPC flows

## Work Breakdown

- `009-todo-future-lineage-and-poll-edges.md`
  - explicit future lineage and poll ownership edges
- `010-todo-request-task-binding-and-cross-process-edges.md`
  - strict request->task and request-parent edges
- `011-todo-future-resource-and-socket-edges.md`
  - structured future->resource identity including sockets
- `012-todo-edge-confidence-and-temporal-model.md`
  - explicit vs derived confidence and time-window semantics

## Dependency Order

1. 009 + 010 + 011 can run in parallel.
2. 012 depends on at least one producer spec being implemented.

## Acceptance Gate (Program)

1. For any request card, graph can render an explicit path:
   `request -> task -> future -> resource -> wake/resume target`
2. Cross-process chain reconstruction does not depend on span-name heuristics.
3. Deadlock candidate list can explain each cycle edge with confidence (`explicit` or `derived`).
4. UI can filter to explicit-only edges.

## Rollout Notes

- Keep schema additive.
- Keep old readers compatible while new fields are optional.
- Instrument producers gradually; UI should degrade gracefully when fields are absent.
