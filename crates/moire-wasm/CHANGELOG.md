# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0](https://github.com/bearcove/moire/compare/moire-wasm-v0.0.0...moire-wasm-v1.0.0) - 2026-03-03

### Added

- add net module (connect/accept instrumentation)
- *(moire-wasm)* add process module stub for API surface parity
- *(moire-wasm)* add missing wasm API surface for roam-core compat

### Other

- More path deps
- Add crate metadata
- Better filtering
- Lotsa frontend work
- User-extensible entity kinds and custom events
- restructure exports to match tokio module layout, add Semaphore
- Don't reexport tokio from moire-wasm maybe...
- Set up captain hooks, edition 2024, README templates
- Align moire::time::timeout with tokio Result API
- Debugging aw yiss
- Consistency of naming with the tokio crate.
- add mac capture API and drop enabled source facade
- rename project from peeps to moire
