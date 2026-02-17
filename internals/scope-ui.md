# Scope UI

Scopes are execution containers (process, thread, task, RPC connection) that entities belong to. They have properties beyond containment — e.g. a connection scope carries flow control state, in-flight count, backpressure info.

This document tracks the planned work for making scopes visible and explorable in the frontend.

---

## 1. Process subgraph layout

Each process scope becomes a visual container in the graph — a bordered/shaded region that groups its entities. The layout runs within each process container. Cross-process edges remain visible between containers.

**Why:** makes multi-process snapshots readable at a glance; cross-process edges (including red/blocked ones) are immediately obvious.

**Open questions:**
- What does the container look like visually? Rounded rect with a label? Dashed border?
- Does the layout engine run per-subgraph or globally with constraints?

---

## 2. Process filter (multi-select dropdown)

Header-level filter: a multi-select dropdown (we have a primitive for this) listing all known processes. Unchecking a process hides it and its entities from the graph.

**Why:** when you have many processes, you may only care about two of them.

---

## 3. Scope inspector integration

When an entity is inspected in the right-side panel, show the scopes it belongs to and their current properties (e.g. connection: 42/64 in-flight, flow control: blocked).

**Why:** entities are often meaningless without knowing their execution context.

---

## 4. Scope table panel

A dedicated panel (separate from the graph) showing all scopes in tabular form — filterable by type (connections, threads, tasks). Columns are scope properties.

**Why:** lets you scan all RPC connections and their states at once, spot the backed-up one without having to click through individual entities.

---

## 5. TX/RX channel node merging

Currently TX and RX ends of a channel are separate nodes. Merge them into a single compound node with a different card style. The card has explicit handles for the TX and RX sides so edges connect to the right side rather than letting the layout engine decide.

**Why:** cleaner graph, immediately obvious that TX/RX are two sides of the same channel.

**Pair detection:**

| Pair | Mechanism | Status |
|------|-----------|--------|
| TX/RX channel ends | `EdgeKind::ChannelLink` (TX → RX) | ready |
| Request/Response | `EdgeKind::RpcLink` (Request → Response) | ready |
| Connection ends | unknown — connections are currently being worked on, revisit later | TBD |

**Open questions:**
- Does node merging require layout engine changes or just rendering?
- What does the merged card look like? Two sections inside one card, with explicit handles on each side?
