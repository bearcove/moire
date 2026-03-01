# Graph Full-Transition Design

This doc is the implementation plan for making graph expand/collapse transitions coherent.

Current pain in real usage:

- Node size changes and graph layout changes are not on one animation timeline.
- Camera pan is delayed independently, so it can drift from what the graph is doing.
- Edges/arrows do not match node motion during transitions.
- Symbolication updates can race with expand transitions and produce invalid intermediate visuals.

The result is exactly what we observed: jumps, retractions, and frames where edges do not correspond to cards.

## Goals

1. One transition timeline for node geometry, edge geometry, and camera motion.
2. No instant layout snap when switching expanded node.
3. No edge/card mismatch during transition.
4. No zero-size intermediate geometry committed to screen.
5. Transition interruptibility: latest user intent wins cleanly.

## Non-goals

1. Perfect physics-like motion.
2. Arbitrary topology morphing in v1 (we can fade for add/remove cases).
3. Rewriting ELK integration.

## Constraints and invariants

From `MANIFESTO.md`: fail fast, loudly, and often.

Required invariants:

1. Rendered node width and height must be `> 0`.
2. Transition source and target geometry must reference the same world coordinate space.
3. During an active transition, render path must use only transition geometry, never mixed transition + live layout geometry.
4. If an invariant fails, abort transition and keep last valid committed geometry.

Hard errors to surface:

- Missing node mapping for an id that is expected to persist across transition.
- Non-finite geometry values.
- Empty/invalid edge polyline data where a visible edge is expected.

## Current architecture summary

Relevant files:

- `frontend/src/components/graph/GraphPanel.tsx`
- `frontend/src/components/graph/GraphViewport.tsx`
- `frontend/src/graph/render/NodeLayer.tsx`
- `frontend/src/graph/render/EdgeLayer.tsx`
- `frontend/src/graph/elkAdapter.ts`
- `frontend/src/graph/canvas/useCameraController.ts`

Current model:

1. `GraphPanel` measures + runs ELK and commits `layout`.
2. `GraphViewport` renders the committed geometry directly.
3. Pan logic in `NodeExpandPanner` is timer-based and separate from geometry transition.
4. Node height reveal is local card animation, not synchronized with world geometry/edges.

## Proposed architecture

Add a graph transition controller in `GraphViewport` that owns a single timeline.

High-level flow:

1. `GraphPanel` computes `nextGeometry` (as today) and passes both `prevGeometry` and `nextGeometry`.
2. `GraphViewport` converts `{prev,next}` into a `TransitionPlan`.
3. `GraphViewport` runs one RAF animation:
   - interpolated nodes/groups
   - interpolated edges
   - interpolated camera target
4. When animation completes, commit `nextGeometry` as stable and clear transition state.

Important: while transition is active, live geometry updates are queued, not rendered immediately.

## Transition data model

Add a small transition model (new module suggested: `frontend/src/graph/transition/model.ts`):

```ts
type TransitionPlan = {
  startedAtMs: number;
  durationMs: number;
  from: GraphGeometry;
  to: GraphGeometry;
  nodeTracks: Map<string, NodeTrack>;
  groupTracks: Map<string, GroupTrack>;
  edgeTracks: Map<string, EdgeTrack>;
  cameraTrack: CameraTrack | null;
};

type NodeTrack = {
  id: string;
  mode: "persist" | "enter" | "exit";
  fromRect: Rect;
  toRect: Rect;
};

type EdgeTrack = {
  id: string;
  mode: "persist" | "enter" | "exit";
  fromPoints: Point[];
  toPoints: Point[];
};
```

## Node/group interpolation

Straightforward per id:

- `x/y/width/height` lerp with easing.
- Persisting nodes/groups keep opacity `1`.
- Enter/exit can fade (`0 -> 1` or `1 -> 0`) in v1.

No partial mixing with non-transition geometry.

## Edge interpolation strategy

Main issue: ELK may change bend point count/order.

V1 robust approach:

1. Normalize every polyline to arc-length parameterization.
2. Resample both polylines to fixed `N` points (for example `N=24`).
3. Interpolate corresponding points.

This avoids brittle “same bend count” assumptions.

Rules:

1. Persisting edge id with both polylines present: morph via resampled points.
2. Enter edge: fade in from target polyline.
3. Exit edge: fade out from source polyline.

If polyline invalid, fail transition and keep last valid frame.

## Camera synchronization

Replace timer-only pan sync with track-based camera motion.

Behavior:

1. Compute desired camera target when expand target changes.
2. Build `cameraTrack` with same duration/easing as geometry transition.
3. During transition RAF, set camera from interpolated camera track.
4. Pan-back uses same mechanism on collapse.

Manual interaction rule stays:

- Any manual pan/zoom cancels restore target immediately.

## Expand reveal synchronization

The card’s internal content reveal should align with world transition.

V1 decision:

1. Keep world geometry transition as source of truth.
2. Keep card internal height reveal but make it a visual detail only.
3. Do not pan on independent timer; pan only from transition controller timeline.

This prevents “card animates now, graph moves later.”

## Scheduling and concurrency

State machine in viewport:

1. `idle`
2. `animating(plan)`
3. `finishing` (commit target, flush queued update)

Update policy:

1. If new geometry arrives in `idle`: start transition immediately.
2. If new geometry arrives while `animating`: replace queued target with latest.
3. On animation end:
   - if queued target exists, start next transition from current visual output.
   - else settle to `idle`.

This handles rapid clicks without chaos.

## Symbolication interaction

Symbolication updates should not force new layout transitions unless transition-relevant inputs changed.

Layout invalidation keys should include only:

1. Node identity and kind that affects measured shell geometry.
2. Edges/topology.
3. Expanded node id.
4. Grouping mode and label mode.

They should not include transient data that does not change geometry intent for transition.

## Performance budget

Target:

1. 60fps on typical graph sizes.
2. No long main-thread blocks during animation.

Tactics:

1. Precompute `TransitionPlan` once per transition.
2. Cache resampled polylines per edge track.
3. Keep interpolation allocations minimal (reuse arrays where possible).
4. Skip transitions for tiny diffs (`epsilon` threshold).

## Rollout plan (multi-session)

### Session 1: Transition infrastructure

1. Add transition model + planner module.
2. Add viewport state machine and RAF loop.
3. Animate nodes/groups only, keep edges static for now.
4. Gate behind `graphFullTransition` feature flag.

Acceptance:

1. No layout snap on node switch.
2. No zero-size node render.

### Session 2: Edge morphing

1. Implement polyline resampling utilities.
2. Animate edge paths + arrowheads with node motion.
3. Add fail-fast validation for edge tracks.

Acceptance:

1. Edges/arrows visually match moving nodes.
2. No path explosions or NaN paths.

### Session 3: Camera unification

1. Move pan/pan-back into transition controller timeline.
2. Remove timer-based pan delay from `NodeExpandPanner`.
3. Preserve manual-interaction cancellation semantics.

Acceptance:

1. Expand feels like one move: reveal + graph + pan.
2. Collapse restore happens only when no manual interaction occurred.

### Session 4: Interruptions and queueing

1. Implement queued-target handling while animating.
2. Ensure latest click wins without flashing intermediate invalid states.
3. Harden with rapid-click tests.

Acceptance:

1. Rapid node switching remains coherent.
2. No visible retraction/collapse glitches.

### Session 5: Cleanup and flag removal

1. Remove old ad-hoc animation paths/timers.
2. Remove feature flag.
3. Final perf pass and docs update.

Acceptance:

1. Stable behavior under symbolication + expand/collapse.
2. Tests and manual scenarios pass.

## Test plan

### Unit tests

1. Polyline resampling returns fixed point counts and finite values.
2. Transition planner maps persist/enter/exit ids correctly.
3. State machine queue behavior (latest target override).

### UI tests

1. Expand single node: no immediate snap to final layout frame.
2. Switch expanded node: no “expand then retract then expand” sequence.
3. Collapse after manual pan: no pan-back.
4. Collapse without manual pan: pan-back occurs.

### Runtime assertions

1. Node dimensions finite and `> 0`.
2. Edge points finite.
3. Transition plan ids consistent with geometry.

## Manual acceptance checklist

1. Collapsed -> expanded:
   - graph moves smoothly
   - edges stay attached
   - camera move is synchronized
2. Expanded A -> expanded B:
   - no instant relayout snap
   - no arrow mismatch during crossfade/morph
3. Symbolication updates during transition:
   - no zero-height graph collapse
4. Rapid clicking:
   - no flashes of empty graph
   - latest clicked node wins predictably

## Open decisions

1. Edge enter/exit style:
   - fade only
   - or short grow/shrink effect along path
2. Duration/easing:
   - fixed duration
   - or distance-aware duration with min/max clamp
3. Whether to keep any internal card-only height animation once full world transition is stable.

## Suggested initial defaults

1. Duration: `220ms`
2. Easing: cubic-out for both geometry and camera
3. Edge resample points: `24`
4. Queue policy: “last update wins”

## Deliverables

1. New transition modules under `frontend/src/graph/transition/`.
2. Refactored `GraphViewport` animation loop and camera sync.
3. Removed timer-only panning path.
4. Tests covering planner, interpolation, queueing, and interaction behavior.

