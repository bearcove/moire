# Dashboard Spec: Granular HTTP API + WS Notify + Manual Refresh

Status: TODO
Owner: TBD
Repo: `peeps`

## Goal
Stop shipping full process dumps to the browser on every update.

Target behavior:
1. Browser fetches data via HTTP GET endpoints only.
2. WebSocket is notification-only (`new data available`), no payload dumps.
3. Dashboard does not auto-apply updates; user must click `Refresh`.

This is required for scale (thousands of tasks/threads) and UI responsiveness.

## Non-goals
- No automatic data reconciliation in browser.
- No full dump streaming over WebSocket.
- No hidden background auto-refresh while viewing.

## Problem Statement
Current model pushes large payloads over WS and repeatedly hydrates the entire app state.
At high cardinality this causes:
- large frame pressure
- heavy JSON parse/decode cost on main thread
- poor interaction latency
- noisy proxy/socket failure modes

## First Principles
- WS is a signal channel, not a data transport.
- Data retrieval is pull-based and explicit.
- UI state changes must be user-driven for expensive views.

## Architecture

### Server responsibilities
1. Maintain latest in-memory snapshots from instrumented processes.
2. Expose granular read endpoints (by concern/tab).
3. Emit WS notifications when snapshot generation changes.

### Browser responsibilities
1. Maintain current view state from last successful refresh.
2. Listen to WS notifications and mark data as stale.
3. Only fetch/apply new data when user clicks `Refresh`.

## WebSocket Contract (Notify-only)
Endpoint: `GET /api/ws`

Message types:
- `{"type":"hello","version":1,"server_time_ms":...,"latest_seq":N}`
- `{"type":"updated","seq":N,"server_time_ms":...,"changed":["tasks","sync",... ]}`

Rules:
- No dump payloads in WS messages.
- `seq` is monotonic on every server state update.
- `changed` is best-effort advisory; client still fetches per active view.

## HTTP API Contract (Granular)
All endpoints are GET and return JSON.

Common query params:
- `since_seq` (optional): client hint for incremental mode.
- `process` (optional): narrow by process name.
- `pid` (optional): narrow by pid.
- `limit`, `offset` (optional): pagination for large collections.

Common response envelope:
- `version: 1`
- `seq: number`
- `server_time_ms: number`
- `data: ...`

### Required endpoints
- `/api/summary`
- `/api/problems`
- `/api/deadlocks`
- `/api/tasks`
- `/api/threads`
- `/api/locks`
- `/api/sync`
- `/api/requests`
- `/api/connections`
- `/api/processes`
- `/api/shm`

### Optional resource endpoints (recommended)
- `/api/tasks/:process/:task_id`
- `/api/locks/:process/:name`
- `/api/sync/mpsc/:process/:name`
- `/api/sync/semaphore/:process/:name`
- `/api/requests/:process/:connection/:request_id`

## Client Behavior Spec

### Refresh model
- On first load:
  - fetch `/api/summary`
  - fetch endpoint(s) for active tab only
- On WS `updated`:
  - set `stale=true`
  - show badge: `New data available` + latest sequence
  - do not mutate current tab data
- On `Refresh` click:
  - fetch active tab endpoint(s)
  - fetch `/api/summary`
  - update local cache and clear stale badge

### No auto-update guarantee
The browser must never apply incoming server state automatically from WS updates.
All state application requires explicit `Refresh` action.

### Caching behavior
- Cache by endpoint + params + `seq`.
- If tab switch occurs without refresh, use last fetched data for that tab.
- Optional: optimistic reuse while showing stale indicator.

## UX Requirements
- Global stale banner in header: `New diagnostics available (seq N)`.
- Refresh button text:
  - idle: `Refresh`
  - stale: `Refresh (new data)`
  - loading: `Refreshing...`
- Show last applied time and sequence in header.

## Performance Requirements
- No endpoint should return full `ProcessDump` unless explicitly requested.
- Response target sizes:
  - summary/problems: small (<100 KB typical)
  - tab-specific collections: bounded via pagination/filtering
- Avoid cross-tab overfetch on refresh.

## Migration Plan
1. Add notify-only WS protocol alongside existing payload WS (compat mode).
2. Introduce granular HTTP endpoints and tab-scoped client fetches.
3. Switch client to manual-refresh model with stale banner.
4. Remove payload WS path once client rollout is complete.

## Rollout Safety
- Feature flag on server: `PEEPS_WS_NOTIFY_ONLY=1`.
- Feature flag on client: `VITE_PEEPS_MANUAL_REFRESH=1`.
- Support both modes for one transition window.

## Acceptance Criteria
- WS traffic contains no dump payloads.
- Clicking `Refresh` is the only path that updates tab data.
- Browser no longer parses full dump payloads for unrelated tabs.
- With 4k+ tasks, dashboard remains interactive while stale.
- Proxy EPIPE noise from large WS payload streaming is eliminated.

## Test Plan
1. Unit tests (server)
- WS emits `updated` on upsert.
- `seq` monotonicity.
- Each endpoint returns only relevant section fields.

2. Integration tests (client)
- Receiving WS `updated` marks stale but does not change rendered data.
- Clicking `Refresh` updates view and clears stale flag.
- Tab switch fetches only tab-scoped endpoint.

3. Manual scenario
- Run high-cardinality workload.
- Verify stale badge appears while data remains stable.
- Verify refresh applies new sequence quickly without full-page jank.

## Open Questions
1. Should `Refresh` be global or tab-scoped by default?
2. Which endpoints need pagination first (`tasks`, `threads`, `sync`)?
3. Do we keep a hidden auto-refresh mode for demos only?

## Handoff Checklist
- [ ] Implement WS notify-only payload schema.
- [ ] Implement `/api/summary` + tab endpoints with envelope + seq.
- [ ] Update frontend store to tab-scoped fetch model.
- [ ] Add stale banner and manual-refresh controls.
- [ ] Add regression tests for no-auto-update behavior.
