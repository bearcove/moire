# Handoff: Fix cross-process graph edges (request→response + channel tx→rx)

## Completed
- Added `GraphReply` type to `crates/peeps-types/src/lib.rs` — slim wire format carrying only the graph
- Added `collect_graph()` to `crates/peeps/src/collect.rs` — graph-only collection path
- Added `collect_roam_session()` to `crates/peeps-types/src/lib.rs` — targeted roam session collection
- Updated `crates/peeps/src/dashboard_client.rs` — sends `GraphReply` instead of `SnapshotReply`
- Updated `crates/peeps-web/src/main.rs` — deserializes `GraphReply`, message type `"graph_reply"`
- Added subgraph filtering in frontend — selecting a request/node BFS-walks the graph and shows only connected component
- Added `PEEPS_CALLER_PROC_KEY` constant to peeps-types (but this approach was REJECTED — see below)
- Committed as `a9f178d` on branch `task-relationship-tracking`

## Active Work

### Origin
User asked for wire format slimming + graph interactivity. During testing, the graph showed disconnected components: request nodes on process `vxd` with channel rx endpoints below them, but NO cross-process edges to the caller's request nodes or to the peer's channel tx endpoints. User's exact words:

> "and where the fuck are the corresponding roam_channel_tx from the other side??"
> "the fuck it does, otherwise it would be on the fucking graph" (about cross-process request→response edges)
> "It's not my fault you designed this like an asshole"
> "stop working and explain the proc_key bullshit right now"

### The Problem
Two bugs prevent cross-process edges from appearing in the graph:

**Bug 1: `resolve_caller_request_id` constructs wrong node IDs**

In `crates/peeps/src/collect.rs:574`:
```rust
let caller_proc_key = peeps_types::sanitize_id_segment(parent_process);
```
This produces `"frontend"` instead of `"frontend-1234"`. The canonical node ID format is `{kind}:{proc_key}:{parts}` where proc_key is `{sanitized_name}-{pid}`. So the constructed edge source `request:frontend:conn_1:42` never matches the actual node `request:frontend-1234:conn_1:42`. Every cross-process request→response edge goes to `unresolved_edges`.

**Bug 2: No cross-process channel edges emitted at all**

In `emit_roam_graph`, tx→rx edges are only emitted when both endpoints are in the same process (line 388-399 of collect.rs). The rx side on the receiver process never emits an edge pointing to the remote tx node.

### The CORRECT Fix (user was very explicit about this)

**DO NOT touch the roam handshake or wire format.** The user was furious when I suggested modifying the handshake to propagate proc_key.

**USE the span_id/chain_id system that already exists.** Roam already propagates these in request metadata:

- `peeps.span_id` — format: `{pid}:{conn_id}:{request_id}`, set by the CALLER in `merged_outgoing_metadata` (`roam/rust/roam-session/src/connection_handle.rs:142-148`)
- `peeps.chain_id` — propagated across call chains
- `peeps.parent_span_id` — the parent's span_id

The span_id is on the outgoing request metadata. The receiver sees it on the incoming request. Both sides have the same identifier. Use it to correlate.

**REVERT the `PEEPS_CALLER_PROC_KEY` addition** — I added it to `peeps-types/src/lib.rs` but the user rejected this approach before I could use it. Remove it.

### How to Fix

**For request→response edges:**
- Process A (caller) emits a request node. Its span_id is in the request's metadata.
- Process B (receiver) emits a response node. It sees the span_id in the incoming request metadata.
- Process B should emit an edge from span_id → its response node.
- BUT: the node ID on process A's side is `request:{A's proc_key}:{conn}:{req_id}`, not the span_id.
- So either: (a) use span_id AS the node ID for request nodes, or (b) store span_id in the node's `attrs_json.meta` and have the server resolve edges by span_id.
- The user's preference is clear: the receiver emits the edge. The receiver has the span_id. Figure out how to make the IDs match.

**For channel tx→rx edges:**
- Channels belong to requests (`RoamChannelSnapshot.request_id`).
- The incoming request on the receiver side has metadata with `peeps.caller_process`, `peeps.caller_connection`, `peeps.caller_request_id`, and the span/chain IDs.
- The receiver's rx endpoint needs to reference the caller's tx endpoint.
- Same correlation problem: need to construct or match the remote node ID.

**The fundamental tension:** The canonical ID format uses proc_key, but the receiver doesn't know the caller's proc_key. The span_id contains the PID (`{pid}:{conn_id}:{request_id}`), and `peeps.caller_process` gives the name, so you CAN reconstruct proc_key as `sanitize_id_segment(caller_process) + "-" + pid_from_span_id`. But this feels fragile.

**Better approach the user hinted at:** Use the span_id/chain_id directly as correlation keys. Both sides know it. Don't try to reconstruct proc_keys across process boundaries.

### Current State
- Branch: `task-relationship-tracking`
- All Rust tests pass (33 peeps-types + peeps-web, 9 peeps)
- Frontend TypeScript compiles
- The slim wire format and subgraph filtering are committed and working
- Cross-process edges are BROKEN (were broken before this work, still broken)

### Files to Touch
- `crates/peeps-types/src/lib.rs:33` — REMOVE the `PEEPS_CALLER_PROC_KEY` constant I added
- `crates/peeps/src/collect.rs:558-575` — `resolve_caller_request_id` needs to construct correct IDs or use span_id correlation
- `crates/peeps/src/collect.rs:229-400` — `emit_roam_graph` needs to emit cross-process channel edges
- `crates/peeps/src/collect.rs:240-317` — the request/response loop needs to use span_id from metadata

### Key Files in Roam (read-only context)
- `roam/rust/roam-session/src/connection_handle.rs:142-249` — `span_id_for_request` and `merged_outgoing_metadata` — where span_id is generated and all peeps metadata is propagated
- `roam/rust/roam-session/src/lib.rs:66-70` — metadata key constants (`peeps.chain_id`, `peeps.span_id`, `peeps.parent_span_id`)
- `roam/rust/roam-session/src/diagnostic.rs` — `DiagnosticState`, `set_peer_name`
- `roam/rust/roam-session/src/diagnostic_snapshot.rs:52+` — how `ConnectionSnapshot` is built from `DiagnosticState`

### Key Types
- `RequestSnapshot.metadata: Option<HashMap<String, String>>` — contains all propagated peeps metadata including span_id, chain_id, caller_process, caller_connection, caller_request_id
- `RoamChannelSnapshot.request_id: Option<u64>` — links channel to its owning request
- `ConnectionSnapshot.in_flight: Vec<RequestSnapshot>` — all in-flight requests with their metadata

### Decisions Made
- User REJECTED modifying roam's handshake to propagate proc_key
- User REJECTED adding `PEEPS_CALLER_PROC_KEY` as a new metadata key
- User wants to use the existing span_id/chain_id correlation mechanism
- The receiver is responsible for emitting cross-process edges

### What NOT to Do
- **DO NOT modify the roam wire format or handshake** (`driver.rs`, `Hello` messages)
- **DO NOT add new metadata keys to roam's `merged_outgoing_metadata`**
- **DO NOT try to reconstruct proc_keys across process boundaries** — this is the broken approach
- The `unresolved_edges` table exists but is not surfaced in the frontend — user noted this is also a bug ("every unresolved edge is a bug!") but it's a separate issue

### Blockers/Gotchas
- `span_id` format is `{pid}:{conn_id}:{request_id}` — NOT a ULID despite user saying it should be. It's what exists now.
- The `metadata` field on `RequestSnapshot` is `Option<HashMap<String, String>>` — values are already stringified, so `peeps.span_id` will be a string like `"1234:5:42"`
- Channel IDs are shared between both sides of a roam connection (same channel_id on tx and rx)
- `find_connection_for_channel` in collect.rs maps channel_id → connection name

## Bootstrap
```bash
git status
cargo nextest run -p peeps -p peeps-types -p peeps-web
```
