# Frontend Investigate MVP Spec

Status: todo
Owner: wg-frontend
Scope: `crates/peeps-web/frontend` (fresh app)

## Stack

- Vite
- Preact
- TypeScript

Fresh UI implementation for `peeps-web`.

## UX requirements (stub phase)

1. Manual "Jump to now" button
2. Basic stuck-request starter query (`elapsed >= 5s`)
3. Table of results only (no graph UI yet)
4. Theme follows OS automatically via CSS `light-dark()`

## Layout (stub)

- Top controls: jump button + current seq indicator
- Main panel: stuck request table
- Optional detail panel: selected request raw fields

## Explicitly out-of-scope (stub phase)

- ELK graph rendering (comes later)
- node inspector cards
- kitchen-sink tabbed dashboards
- SQL editor/runner UI
- auto-refresh or live streaming UI

## No auto-update rule

- App state is pinned to selected `seq`.
- Arrival of newer seq only updates "latest available" indicator.
- Graph/data remain unchanged until user clicks "Jump to now".

## Stub strategy

- Keep UI intentionally small to avoid kitchen sink.
- Prioritize data correctness and queryability over presentation.

## Acceptance criteria

1. User can find stuck requests quickly with starter query.
2. UI uses OS-driven theme through `light-dark()` with no manual toggle.
3. UI never auto-mutates selected snapshot context.
