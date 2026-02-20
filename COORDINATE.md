# Coordination Board

## Context
- Goal: Prototype a secondary timeline-first investigation mode driven by `events` table queries.
- Constraints: Keep graph mode intact, use `/api/sql` only, and isolate spike behind a simple toggle.

## Active Claims
- Agent: Codex (this session)
- Status: Completed
- Files touched:
  - `/Users/amos/bearcove/moire/crates/moire-web/frontend/src/App.tsx`
  - `/Users/amos/bearcove/moire/crates/moire-web/frontend/src/api.ts`
  - `/Users/amos/bearcove/moire/crates/moire-web/frontend/src/types.ts`
  - `/Users/amos/bearcove/moire/crates/moire-web/frontend/src/styles.css`
  - `/Users/amos/bearcove/moire/crates/moire-web/frontend/src/components/TimelineView.tsx` (new)

## Task Checklist
- [x] Inspect current graph selection/focus flow in `App.tsx`
- [x] Inspect `/api/sql` constraints and `events` schema
- [x] Add timeline event query helper(s) via `/api/sql`
- [x] Add isolated timeline UI component
- [x] Add graph/timeline mode toggle in `App.tsx`
- [x] Wire click event -> focus corresponding graph node
- [x] Add lightweight timeline styles
- [x] Run frontend build/typecheck
- [x] Write production indexing/query tuning notes

## Progress Log
- 2026-02-16: Started spike, mapped frontend state and graph focus plumbing.
- 2026-02-16: Confirmed runtime `events` table exists and is queryable through `/api/sql`.
- 2026-02-16: Added frontend API helpers for recent timeline events/process options and created isolated `TimelineView` component.
- 2026-02-16: Wired `App.tsx` mode toggle (`graph` / `timeline`) with timeline query controls (process + window), and click-through to graph focus.
- 2026-02-16: Verified with `npm run build` in `crates/moire-web/frontend`.

## Notes For Other Agents
- The spike is isolated to timeline-prefixed helpers/state/components and a single mode toggle branch in `App.tsx`.
- Graph mode path remains the existing `GraphView` render branch unchanged.
- Follow-up still needed: model internal roam queue hop (`call_raw_with_channels` -> runtime channel) as explicit edge(s) so queued backpressure is visible in graph.
