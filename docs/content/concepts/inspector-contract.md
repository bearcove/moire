+++
title = "Inspector Contract"
weight = 8
+++

The frontend inspector uses a strict canonical attribute contract. It does not read legacy aliases.

## Canonical fields

Inspector common fields read only:

- `created_at` (required, Unix epoch ns)
- `source` (required)
- `method` (optional)
- `correlation` (optional)

Node identity and process come from node envelope fields:

- `id` (required)
- `process` (required)

## Timeline origin

Timeline origin uses:

1. `created_at` if present and sane for the event window
2. first timeline event timestamp otherwise

Sanity guard:

- if `created_at > first_event_ts`, use first event
- if `first_event_ts - created_at > 30 days`, use first event

No fallback alias keys are used.
