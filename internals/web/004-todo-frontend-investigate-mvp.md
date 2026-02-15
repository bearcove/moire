# Frontend Investigate MVP Spec

Status: todo
Owner: wg-frontend
Scope: `crates/peeps-web/frontend` (fresh app)

## Stack

- Vite
- Preact
- TypeScript

Fresh UI implementation for `peeps-web`.

## UX requirements

1. Snapshot picker (seq list)
2. Manual "Jump to now" button
3. Stuck RPC list (`>5s` configurable)
4. Graph panel for selected request
5. Node inspector with attrs + stacktrace (if present)
6. Global fuzzy search over node IDs/labels/attrs

## Layout (v1)

- Left rail: snapshots + stuck requests + search
- Center: graph canvas
- Right rail: inspector

## Graph interactions

- click node selects in inspector
- click edge shows edge attrs
- filter controls:
  - by node kind
  - by process

## No auto-update rule

- App state is pinned to selected `seq`.
- Arrival of newer seq only updates "latest available" indicator.
- Graph/data remain unchanged until user clicks "Jump to now".

## MVP rendering strategy

- Start with simple force or layered layout (library choice open).
- Prioritize correctness + inspectability over visual polish.

## Acceptance criteria

1. Can identify all requests stuck >5s from selected seq.
2. Selecting one request renders a connected graph and details.
3. User never loses point-in-time context due to background updates.
