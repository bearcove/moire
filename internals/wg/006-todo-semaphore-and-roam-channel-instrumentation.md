# 006 TODO - Tokio Semaphore + Roam RPC Channel Instrumentation

Status: TODO
Owners: Unassigned
Related repos: `vixen`, `roam`, `peeps`

## Goal
Add first-class wait-graph-quality instrumentation for:
1. Tokio semaphores (especially in Roam and Vixen hot paths)
2. Roam RPC channels (Roam-level channels, not generic Tokio channels)

The immediate objective is better deadlock and wait-chain visibility while cycle-detection improvements are underway.

## Why this exists
Current deadlock output is noisy because many cycles are synthetic future/task loops. While that is fixed, we still need stronger signal from real contention points:
- semaphore permit starvation/convoys
- request/response channel stalls inside Roam transport/session pipelines

This spec focuses only on additive instrumentation and graph ingestion, so multiple teams can ship in parallel.

## Non-goals
- No redesign of deadlock SCC algorithm in this phase.
- No breaking wire protocol changes for Roam RPC.
- No broad replacement of all Tokio primitives at once.
- No UI redesign beyond adding rows/links needed to expose new data.

## Deliverables
1. Stable snapshots for semaphore and Roam channel state in process dumps.
2. Wait-graph edges derived from these snapshots.
3. Dashboard visibility with resource links and severity hints.
4. Integration in both `roam` and `vixen` codepaths that matter for current deadlock investigations.
5. Regression tests + fixture tests proving signal quality.

## Workstreams

## WS1 - Semaphore instrumentation (primitive + wrappers)

### Scope
Instrument semaphore lifecycle events with task context:
- create semaphore
- acquire started
- acquire granted
- acquire dropped/cancelled/timeout
- permit released

### Data model requirements
Each semaphore snapshot must include at least:
- `name`
- stable `semaphore_id` (process-local, monotonic)
- `permits_total`
- `permits_available`
- `waiters`
- `acquires`
- `avg_wait_secs`
- `max_wait_secs`
- `creator_task_id`/`creator_task_name`
- `age_secs`

If not already present, add:
- `top_waiter_task_ids` (bounded list, e.g. top 8 by wait age)
- `oldest_wait_secs`

### Implementation notes
- Use `peeps-sync` semaphore wrapper as canonical source.
- Prefer explicit wrapper adoption in target codepaths rather than global monkey-patching.
- For acquire operations, capture pending start time and task_id from peeps task context.
- Ensure cancellation paths decrement waiter counts correctly.

### Target adoption points
- `roam-session` request concurrency semaphore(s)
- `vxd` executor / bounded parallelism controls
- `vx-store` critical semaphores used by request handlers

## WS2 - Roam RPC channel instrumentation

### Scope
Instrument Roam-level channels used by RPC/session internals, not generic `tokio::sync::mpsc`.

Track per-channel:
- channel identity (`channel_id`, `name`, `kind`)
- producer/consumer roles
- in-flight counts / queue depth where available
- pending sender/receiver waiters
- created_by task
- closed state and closure reason (if available)
- age

### Context propagation requirements
Every channel activity should attempt to carry:
- `task_id`
- `task_name`
- `request_id` (if in RPC context)
- optional trace metadata map (`trace_id`, `span_id`, etc.)

When context is absent, record explicitly as unknown (do not drop event).

### Implementation notes
- Instrument in `roam-session` internals (`channel.rs`, driver/connection plumbing).
- Keep API additive and backward compatible.
- Reuse existing peeps snapshot extension points for `roam` section in `ProcessDump`.

## WS3 - Wait graph ingestion

### Scope
Convert semaphore/channel snapshots into graph nodes and blocking edges.

### Required node/edge semantics
- Semaphore node: `sem:<name>#<id>`
- Channel node: `roamch:<name>#<id>`

Edges:
- `task -> semaphore` (`TaskWaitsOnResource`) when waiting to acquire
- `semaphore -> task` (`ResourceOwnedByTask`) for current permit holders (if represented)
- `task -> roam-channel` when blocked on send/recv
- `roam-channel -> task` owner/handler edge when another task is expected to make progress

Severity hint recommendations:
- waiter age > 1s => hint 1
- waiter age > 10s => hint 2
- waiter age > 30s => hint 3

### Guardrails
- Do not encode purely structural/provenance edges as blocking.
- Avoid self-loop artifacts unless truly blocking.
- Deduplicate repeated identical edges within a snapshot.

## WS4 - Dashboard exposure

### Scope
Expose data in existing tabs and resource detail pages.

Minimum UI additions:
- `Sync` tab: semaphore rows with severity badge
- Roam channel rows with severity badge
- All resource names rendered as clickable resource tags (icon + label) with stable routes:
  - `/semaphores/:process/:id`
  - `/roam-channels/:process/:id`

Task details must list full interactions:
- locks
- channels (tokio + roam)
- semaphores
- RPC requests

## WS5 - Validation and rollout

### Tests (required)
1. Unit tests for wrapper bookkeeping:
- waiter increment/decrement correctness
- cancellation paths
- closed-channel behavior

2. Wait-graph tests:
- semaphore contention graph edges appear
- Roam channel blocked send/recv edges appear
- no synthetic self-cycle introduced by these edges

3. Integration smoke (manual acceptable initially):
- run peeps dashboard with vixen workload
- verify non-zero semaphore/channel resources
- verify resources are clickable and deep-linkable

### Rollout plan
1. Land primitive instrumentation in `peeps` + `roam` behind default-on diagnostics.
2. Adopt wrappers in high-value codepaths in `roam` then `vixen`.
3. Enable graph ingestion and dashboard rows.
4. Tune thresholds from observed traces.

## Parallelization plan
These can run in parallel safely:
- Team A: WS1 semaphore wrapper + snapshots
- Team B: WS2 Roam channel snapshots + context metadata
- Team C: WS3 wait-graph ingestion + tests
- Team D: WS4 dashboard routes/tags/views

Merge order recommendation:
1. WS1 + WS2 (data producers)
2. WS3 (graph)
3. WS4 (UI)
4. WS5 threshold tuning

## Acceptance criteria
- `peeps` dumps include semaphore and Roam channel snapshots with stable IDs.
- Wait graph contains corresponding nodes/edges for real blocked waits.
- Dashboard can navigate to each resource via stable URL.
- At least one real vixen/roam contention scenario is visible as a resource chain.
- No large spike in trivial deadlock candidates attributable to new instrumentation.

## Open questions
1. Should semaphore permit holders be represented explicitly when permits >1?
2. For Roam channels, what is the canonical `channel_id` type (u64 local vs tuple key)?
3. Should request/trace metadata be serialized fully or redacted/whitelisted?
4. Should stalled-but-idle channels be info instead of warn by default?

## Handoff checklist for assignees
- [ ] Link PRs to this spec.
- [ ] Add fixture proving at least one meaningful wait chain from semaphore/channel data.
- [ ] Include before/after dashboard screenshots for one contention scenario.
- [ ] Document any added env flags.
