# Peeps Strictness Manifesto

## Principle

Fail fast, loudly, and often.

If an assumption exists, validate it immediately. If validation fails, stop and surface a hard error with explicit context. Do not silently coerce, default, truncate, or "best effort" through invalid state.

## Rules

1. No silent fallbacks for required data.
2. No fabricated placeholders for missing identity, linkage, or protocol fields.
3. No "unknown", "0", empty-list, or null defaults where correctness depends on a real value.
4. Treat protocol and state mismatches as errors, not heuristics.
5. Preserve causality: emit canonical IDs and reject ambiguous wiring.
6. Prefer explicit crashes/panics/exceptions in invalid internal states over corrupted output.
7. Error messages must identify the violated invariant and relevant identifiers.

## Engineering Stance

Half-working behavior is failure. Correctness over convenience. Strictness over guesswork.

