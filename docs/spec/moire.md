# Moire Specification

Moire is a low-overhead instrumentation library for production Rust systems built on Tokio.

It provides structured observability into async task execution, lock contention, and channel behavior — with optional live push to a dashboard server.

---

## Configuration

### Instrumented process

An instrumented process is any binary that depends on `moire` with the `diagnostics` cargo feature enabled.

> r[config.dashboard-addr]
> The instrumented process reads `MOIRE_DASHBOARD` at startup. If set to a non-empty `<host>:<port>` string, it initiates a persistent TCP push connection to that address.

> r[config.dashboard-feature-gate]
> If `MOIRE_DASHBOARD` is set but the `diagnostics` feature is not enabled, the process MUST emit a warning to stderr and MUST NOT attempt to connect.

> r[config.dashboard-reconnect]
> If the connection to the dashboard is lost, the process MUST attempt to reconnect after a delay. It MUST NOT crash or log an unrecoverable error on connection failure.

### moire-web server

`moire-web` is the dashboard server. It accepts TCP pushes from instrumented processes and serves an HTTP investigation UI.

> r[config.web.tcp-listen]
> `moire-web` reads `MOIRE_LISTEN` for the TCP ingest address. Default: `127.0.0.1:9119`.

> r[config.web.http-listen]
> `moire-web` reads `MOIRE_HTTP` for the HTTP UI address. Default: `127.0.0.1:9130`.

> r[config.web.db-path]
> `moire-web` reads `MOIRE_DB` for the SQLite database file path. Default: `moire-web.sqlite`.

> r[config.web.vite-addr]
> In dev mode, `moire-web` reads `MOIRE_VITE_ADDR` for the Vite dev server proxy address.
