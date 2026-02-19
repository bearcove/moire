# Patch + Reflection Plan

## Problem

Right now, runtime db updates are "one method per entity kind", with hand-written comparisons and hand-written field assignment.

That gives us a few bad outcomes:

- every new field means new manual compare/update code
- update logic is spread across many methods, so behavior drifts
- domain transition logic, persistence serialization, and change-stream emission are all mixed in one place
- callers either pass too much state every time, or we keep adding narrow ad-hoc methods

This is exactly what `update_once_cell_state(...)` shows.

## First answer: is our state a clone of the initial thing?

No.

`RuntimeDb` owns the canonical mutable state now (`entities`, `scopes`, `edges`, etc). That state is incrementally mutated over time. It is not "a clone of the initial entity" in practice.

What we *do* clone frequently is identifiers and serialized payloads when pushing changes.

So the core issue is not "we only have an initial clone". The issue is that mutation API shape is too manual and too kind-specific.

## What we can do

### Option A: Keep manual per-kind update methods

We can improve ergonomics with helpers (`mutate_entity`, `push_entity_upsert`) but still keep one method per kind.

Pros:

- minimal change risk
- explicit logic per kind

Cons:

- still lots of boilerplate
- still easy to forget fields
- still hard to scale

### Option B: Full replace updates (pass full struct each time)

Callers build full new state and db compares/replaces.

Pros:

- very simple write path

Cons:

- callers must track all fields and current state
- too expensive mentally and often computationally
- encourages stale/partial reconstructions

### Option C: JSON diff + reflection-driven apply (recommended)

Mutate the typed value in memory, serialize before/after to `facet_value::Value`, diff, and persist
only the patch.

Example shape:

- `before = facet_value::to_value(entity.body)`
- mutate closure runs on `&mut entity.body`
- `after = facet_value::to_value(entity.body)`
- `patch = diff(before, after)` (JSON-patch-like)
- append `Change::PatchEntity { id, patch }` when non-empty

Pros:

- no per-field comparison boilerplate
- strict kind checking can fail fast
- easy to add fields
- no custom patch schema needed at first

Cons:

- serialization/diff overhead
- patch format becomes part of the stream contract

## Recommended split of responsibilities

Regardless of patch mechanism, we should split `RuntimeDb` responsibilities:

1. transition/apply: mutate typed state and return changed/not-changed
2. journaling: serialize and push changes in one place
3. runtime wiring: globals, task lookup, process-scope hooks outside core db

Patch reflection fits this split naturally.

## Proposed incremental rollout

1. Add one generic helper in db:
   - mutate entity by id
   - if changed, append one `PatchEntity` change
2. Route `update_once_cell_state` through this helper first
4. Add strict tests:
   - kind mismatch fails hard
   - no-op patch does not emit change
   - changed patch emits exactly one patch change
5. Migrate channel/semaphore/notify updates one by one

## Guardrails

- no silent fallback on kind mismatch
- no best-effort partial apply on unknown fields
- one canonical place for before/after serialization and diff
- patch application must be deterministic and explicit

## JSON-first API draft

This is intentionally not codec-pluggable. We embrace `facet_value::Value`.

```rust
pub enum MutateOutcome {
    NoChange,
    Changed { patch: facet_value::Value },
}

impl RuntimeDb {
    pub fn mutate_entity_body(
        &mut self,
        id: &EntityId,
        expected_kind: EntityBodyKind,
        f: impl FnOnce(&mut EntityBody),
    ) -> Result<MutateOutcome, MutateError> {
        let entity = self.entities.get_mut(id).ok_or(MutateError::NotFound)?;
        if entity.body.kind() != expected_kind {
            return Err(MutateError::KindMismatch);
        }

        let before = facet_value::to_value(&entity.body).map_err(MutateError::Encode)?;
        f(&mut entity.body);
        let after = facet_value::to_value(&entity.body).map_err(MutateError::Encode)?;

        let patch = peeps_patch::diff(&before, &after);
        if peeps_patch::is_empty(&patch) {
            return Ok(MutateOutcome::NoChange);
        }

        self.push_change(InternalChange::PatchEntity {
            id: EntityId::new(id.as_str()),
            patch_json: facet_json::to_vec(&patch).map_err(MutateError::Encode)?,
        });
        Ok(MutateOutcome::Changed { patch })
    }
}
```

Handler shape:

```rust
once_cell_handle.mutate(|body| {
    let EntityBody::OnceCell(cell) = body else { unreachable!() };
    cell.waiter_count = waiter_count;
    cell.state = state;
})?;
```

## Crate split

- `peeps-patch`: `diff`, `apply`, `is_empty` over `facet_value::Value`
- `peeps-db-core`: typed runtime state + mutate/commit + change journal
- `peeps-db-runtime`: global static/sync wrappers and process/task wiring
