# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0](https://github.com/bearcove/moire/compare/moire-web-v0.0.0...moire-web-v1.0.0) - 2026-03-03

### Other

- Add crate metadata
- Data test
- Fix clippy errors in mcp/mod.rs
- Extract source_context into moire-source-context crate
- Move context line splitting and cut-marker collapsing to backend
- Move tests into source_context folder
- Fix generated TS docs and frontend type mismatches
- visual polish on graph node chips and cards
- Add xtask install and bundled frontend serving
- return 3-frame source stacks with app-frame skip semantics
- prefer caller wait-sites and keep target code visible
- render MCP results as markdown with filtered callsites
- wait for symbolication in MCP source-bearing tools
- add moire_help and first-run guidance
- test deadlock SCC and wake-source heuristics
- stabilize graph output ordering
- add wait-site source context to entity edges
- embed edge wait-site source in graph outputs
- add deadlock-focused MCP graph tools
- add direct MCP server surface
- More frontend fixes
- Add scope color dot to graph nodes
- Zoom debug
- Future tweaks
- summaries / helpful stuff in code nodes
- Add enclosing function context to source preview
- Free ELK port placement for channel/rpc pair nodes
- Fix pan-back restoring to wrong camera position
- Fix drag-to-pan closing active node
- Resolve /rustc/{hash} paths before sending to frontend
- fix closing-brace scope capturing entire file
- Extract FrameList component with Show system frames toggle
- CSS fixes
- theme-derived
- Try to use theme colors
- Generate arborium theme as CSS variables instead of hardcoded colors
- Smarter extract_target_statement + kind-aware node cards
- context_line + kind-aware source display
- lang-aware-context feature
- Tree-sitter display range for compact backtrace frames
- Source preview in graph nodes
- More tuning
- serve theme
- Add source preview API and wire up frontend plumbing
- Restore stacktraces
- Introduce stable process IDs and simplify DB partitioning
- Use backtrace IDs directly in snapshot symbolication flow
- Tighten CutId and SessionId API usage
- Type next connection ID through app bootstrap
- Remove ID getters and rely on Display and SQL traits
- Type backtrace IDs in moire-web and add ID display format
- Type moire-web conn_id to SQL bind boundaries
- Use typed ConnectionId at moire-types API boundary
- Add typed IDs and typed addresses across runtime and web
- Panic on u64->i64 SQLite overflow
- Fix module-id manifest mapping with typed wire IDs
- migrate delta batch persistence to rusqlite-facet
- migrate core persistence writes to rusqlite-facet
- use rusqlite-facet in db schema queries
- migrate symbolication SQL to rusqlite-facet
- use rusqlite-facet in db query helpers
- move app bootstrap and router wiring into app module
- extract dev proxy and vite lifecycle
- extract tcp ingest and handshake logic
- move sql route handlers into api module
- extract connections and cut API handlers
- extract recording API handlers
- extract snapshot API and stream handlers
- move shared app state types into app module
- use rusqlite-facet in snapshot repository
- reduce symbolication stream log noise
- extract snapshot frame repository
- keep next() as only public constructor
- centralize 16+37 layout in trace-id types
- use reusable JS-safe u64 id macro and add FrameId type
- keep snapshot payload free of frame catalogs
- extract vite proxy forwarding into proxy module
- extract recording session state and helpers
- move symbolication engine out of main
- trim symbolication debug log verbosity
- move snapshot frame catalog into snapshot module
- move cut and delta persistence into db layer
- move sql api logic into api module
- extract db persistence helpers from main
- move schema and sql query helpers into db module
- introduce Db facade and remove db-path plumbing in moire-web
- start moire-web modularization with skeleton and shared utils
- Fix all clippy warnings across workspace
- Set up captain hooks, edition 2024, README templates
- Add verify annotations and tests for ID/snapshot rules
- Sync spec with streamed symbolication and stable IDs
- Demangle symbolicated frame names
- Fix symbolication probe address mapping for ASLR
- Trace every symbolication step and loader call
- Log manifest mismatch causes for unresolved symbolication
- Add per-frame symbolication debug success logs
- Retry stale symbolication rows and enforce handshake ordering
- Replace fake eager path with real frame symbolication and stream completion
- Stream deduped frame symbolication updates after snapshot
- ship full resolved backtraces in cut responses
- Debugging aw yiss
- Improve moire-web cut/snapshot observability and DB lock handling
- Unify web ingest with moire_wire ClientMessage
- Consistency of naming with the tokio crate.
- No more mention of sources
- checkpoint backtrace-id migration and snapshot/source cleanup
- add backtrace record ingest/emission and eager symbolication pipeline
- rework frontend spec for backtraces, drop composite IDs
- set up tracey and document configuration surface
- rename project from peeps to moire
