# Frontend Investigate MVP Spec

Status: todo
Owner: wg-frontend
Scope: `crates/peeps-web/frontend` (fresh app)

## Stack

- Vite
- Preact
- TypeScript

Fresh UI implementation for `peeps-web`.

## UI direction (concrete)

Visual style:
- dense, technical, low-noise surface
- strong contrast, restrained accents by severity/state
- monospace-forward data presentation
- OS theme only (`light-dark()`), no manual theme toggle

Primary screen structure:
- header bar
  - title: `peeps web`
  - live snapshot indicator
  - `Jump to now` button
- tab bar
  - only `Requests` (single tab for this phase)
- body split (desktop):
  - left: stuck request list/table
  - center: ELK graph prototype
  - right: inspector panel
- body stack (mobile):
  - requests list -> graph -> inspector (vertical)

## UX requirements (stub phase)

1. Manual "Jump to now" button
2. Basic stuck-request starter query (`elapsed >= 5s`)
3. Single top-level tab: `Requests`
4. ELK graph prototype using mock data
5. Node-type icons in graph and lists
6. Hover cards for quick node/edge details
7. Click node/edge opens side inspector
8. Theme follows OS automatically via CSS `light-dark()`

## Layout (stub)

- Top controls: jump button + current seq indicator
- Tab row: only `Requests`
- Requests panel: stuck request table + ELK prototype area
- Right side inspector: selected request/node/edge details

## Component contract

Requests table:
- columns: method, process->peer, elapsed, task label, status
- sort: elapsed descending by default
- row click: focuses corresponding graph subgraph + opens inspector

ELK prototype panel (mock data allowed):
- directional left-to-right flow
- node chips with icon + label + short id suffix
- edge labels on hover (kind + duration/count)
- basic pan/zoom

Inspector panel:
- section 1: identity (id, kind, process)
- section 2: key attrs (parsed from attrs_json)
- section 3: related edges (incoming/outgoing grouped)
- section 4: raw JSON fallback

Hover cards:
- node hover: kind icon, label, process, hot metrics
- edge hover: edge kind, source->target, duration/count
- never require click for quick triage facts

## Icon mapping (v1)

- process: grid
- task: check-circle
- future: clock
- request: arrow-right-left
- lock: lock
- semaphore: key
- mpsc/oneshot/watch: radio
- roam_channel: route
- socket: plug
- thread: cpu

## Explicitly out-of-scope (stub phase)

- multiple feature tabs beyond `Requests`
- kitchen-sink resource dashboards
- SQL editor/runner UI
- auto-refresh or live streaming UI

## No auto-update rule

- App state is pinned to selected `seq`.
- Arrival of newer seq only updates "latest available" indicator.
- Graph/data remain unchanged until user clicks "Jump to now".

## Stub strategy

- Keep product scope intentionally small (Requests-only), but allow richer interaction prototyping.
- Use mock data for ELK/hover/inspector until backend query coverage is ready.

## Interaction quality bar

- no hidden essential actions
- keyboard support: up/down list, enter to focus, esc to clear selection
- preserve selection when new snapshot is loaded where possible (by stable id)
- empty states must be explicit (\"no stuck requests\", \"no graph data\")

## Acceptance criteria

1. User can find stuck requests quickly with starter query.
2. `Requests` is the only top-level tab.
3. ELK mock prototype supports icons, hover cards, and side inspector interactions.
4. UI uses OS-driven theme through `light-dark()` with no manual toggle.
5. UI never auto-mutates selected snapshot context.
