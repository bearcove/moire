# peeps sync migration guide (single source of truth)

When your async service freezes, the question is not "did we preserve every old field?".
The question is "can we explain who is waiting on whom, and why this wait is not resolving?".

This guide adopts `peeps-types` as the source of truth and keeps only information that helps deadlock diagnosis.

## Model-first stance

The migration is a model shift, not a compatibility exercise.

- `Entity` holds long-lived runtime objects.
- `Edge` holds relationships.
- `Event` holds transitions/outcomes.
- `PTime` (`birth`, event `at`) is canonical time.

## Canonical decisions

1. `created_at_ns` is conceptually replaced by `birth`.
- Use `Entity.birth` (`PTime`) as the creation-time field.
- Do not preserve absolute Unix ns by default.

2. Timer/sleep instrumentation is intentionally out of scope.
- `sleep` / `interval` / `interval_at` telemetry is dropped.
- Reason: it did not help deadlock detection.

3. Keep only deadlock-relevant diagnostics.
- If a field does not improve deadlock explanation, drop it.
- If uncertain, keep briefly in `meta`, then remove if unused.

## Classification buckets

Every legacy detail should land in exactly one bucket:

1. `Top-level field`
2. `Body field`
3. `meta`
4. `Event`
5. `Edge`
6. `Likely oversight/blindspot`

## Cross-cutting mapping rules

| Legacy detail | Bucket | Target |
| --- | --- | --- |
| endpoint id (`tx_node_id`, `rx_node_id`, `node_id`) | `Top-level field` | `Entity.id` |
| location/callsite | `Top-level field` | `Entity.source` |
| creation time (`created_at_ns`) | `Top-level field` | `Entity.birth` |
| label/name | `Top-level field` | `Entity.name` |
| runtime typed state | `Body field` | `EntityBody::*` |
| send/recv/close/wait transitions | `Event` | typed sync events |
| tx-rx pairing | `Edge` | `EdgeKind::ChannelLink` |
| active blocking dependency | `Edge` | `EdgeKind::Needs` |
| closure causality | `Edge` or `Event` | `ClosedBy` and/or `ChannelClosed` |
| high-watermark, utilization, counters | `meta` | optional diagnostics only |
| freeform close-cause strings | `meta` | optional compatibility/debug context |
| internal waiter bookkeeping lists | `Likely oversight/blindspot` | usually noise in final graph |

## Primitive mapping (deadlock-focused)

| Primitive | Keep in top-level/body | Keep as events/edges | Optional `meta` | Drop |
| --- | --- | --- | --- | --- |
| `mpsc` / unbounded channel | ids, source, name, lifecycle, capacity | sent/received/wait/closed events, `ChannelLink`, `Needs` | high-watermark, waiter/sender counters, utilization | legacy string parity |
| `oneshot` | ids, source, name, oneshot state | send/recv/closed events, `ChannelLink`, `Needs` | freeform drop reasons if needed | anything not improving closure/wait reasoning |
| `watch` | ids, source, name, lifecycle/watch details | send/changed/wait events, `ChannelLink`, `Needs` | receiver counts, change counters | noisy legacy mirrors |
| `semaphore` | ids, source, name, max/handed-out permits | acquire/try-acquire outcomes, wait events, `Needs` | watermarks and wait aggregates | internal per-waiter scratch state |
| `notify` | ids, source, name, waiter count | notify/wait transitions, `Needs` | wakeup/notify counters and aggregates | internal waiter-start lists |
| `oncecell` | ids, source, name, oncecell state | init attempt/completion events, `Needs` | init duration/retry counters | internal bookkeeping noise |
| timers (`sleep`, `interval`, `interval_at`) | none | none | none | intentionally excluded |

## Typed event/edge baseline

Use this as the minimum useful stream:

- Events:
  - `ChannelSent`
  - `ChannelReceived`
  - `ChannelClosed`
  - `ChannelWaitStarted`
  - `ChannelWaitEnded`
- Edges:
  - `Needs`
  - `ChannelLink`
  - `ClosedBy`

## What disappears on purpose

- Absolute timestamp compatibility (`created_at_ns` as canonical field).
- Timer/sleep/interval entity and tick telemetry.
- Legacy string-event parity as a migration objective.
- Counter dumps that do not help explain blocked dependency chains.

## What we still must gather

1. Wait boundaries around blocking operations.
2. Causal closure transitions.
3. Stable relationship edges (`ChannelLink`, `Needs`, `ClosedBy`).
4. Minimal `meta` only when it materially helps incident analysis.

## Implementation order

1. Build entities with correct `EntityBody` and canonical top-level fields.
2. Emit structural edges (`ChannelLink`) and wait edges (`Needs`).
3. Replace legacy string events with typed events.
4. Add only justified `meta` keys.
5. Delete or ignore everything else.

That is the migration contract: preserve deadlock signal, not legacy shape.
