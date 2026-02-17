//! HTTP API endpoints: POST /api/take-snapshot, GET /api/snapshot-progress,
//! GET /api/connections, POST /api/process-debug, GET /api/process-debug-result/{result_id},
//! POST /api/sql
//!
//! SQL enforcement: authorizer, progress handler, hard caps.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::time::Instant;

use axum::extract::{Path as AxumPath, State};
use axum::response::IntoResponse;
use axum::http::StatusCode;
use axum::http::header;
use axum::Json;
use rusqlite::types::Value;
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tracing::{debug, error, info, warn};

use crate::AppState;

// ── Constants ────────────────────────────────────────────────────

const MAX_ROWS: usize = 5000;
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_EXECUTION_MS: u64 = 750;
/// Progress handler callback interval (in SQLite virtual-machine ops).
const PROGRESS_HANDLER_OPS: i32 = 1000;
const PROCESS_DEBUG_OUTPUT_PREFIX: &str = "process-debug-result";

static PROCESS_DEBUG_OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(1);

// ── Scoped TEMP VIEW tables ──────────────────────────────────────

/// Tables that get scoped TEMP VIEWs and are blocked from direct access.
/// Each entry is (table_name, columns_excluding_snapshot_id).
const SCOPED_TABLES: &[(&str, &str)] = &[
    ("nodes", "id, kind, process, proc_key, attrs_json"),
    ("edges", "src_id, dst_id, kind, attrs_json"),
    (
        "snapshot_processes",
        "process, pid, proc_key, status, recv_at_ns, error_text",
    ),
];

// ── Request/response types ───────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TakeSnapshotResponse {
    pub snapshot_id: i64,
    pub captured_at_ns: i64,
    pub requested: usize,
    pub responded: usize,
    pub timed_out: usize,
    pub error: usize,
}

#[derive(Debug, Serialize)]
pub struct SnapshotProgressResponse {
    pub active: bool,
    pub snapshot_id: Option<i64>,
    pub requested: usize,
    pub responded: usize,
    pub pending: usize,
    pub responded_processes: Vec<String>,
    pub pending_processes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionsResponse {
    pub connected_processes: usize,
    pub can_take_snapshot: bool,
    pub processes: Vec<ConnectedProcessInfo>,
}

#[derive(Debug, Serialize)]
pub struct ConnectedProcessInfo {
    pub proc_key: String,
    pub process_name: String,
}

#[derive(Debug, Serialize)]
pub struct SnapshotProcessInfo {
    pub process: String,
    pub pid: Option<i64>,
    pub proc_key: String,
    pub status: String,
    pub recv_at_ns: Option<i64>,
    pub error_text: Option<String>,
    pub command: Option<String>,
    pub cmd_args_preview: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotProcessesResponse {
    pub snapshot_id: i64,
    pub processes: Vec<SnapshotProcessInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ProcessDebugRequest {
    pub snapshot_id: i64,
    pub proc_key: String,
    pub action: String,
    #[serde(default)]
    pub run: bool,
}

#[derive(Debug, Serialize)]
pub struct ProcessDebugResponse {
    pub snapshot_id: i64,
    pub process: String,
    pub proc_key: String,
    pub pid: Option<i64>,
    pub action: String,
    pub command: String,
    pub status: String,
    pub status_message: Option<String>,
    pub command_output: Option<String>,
    pub command_exit_code: Option<i32>,
    pub result_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SqlRequest {
    pub snapshot_id: i64,
    pub sql: String,
    #[serde(default)]
    pub params: Vec<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct SqlResponse {
    pub snapshot_id: i64,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<JsonValue>>,
    pub row_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiError {
    error: String,
}

fn api_error(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { error: msg.into() }))
}

// ── POST /api/take-snapshot ──────────────────────────────────────

pub async fn api_take_snapshot(
    State(state): State<AppState>,
) -> Result<Json<TakeSnapshotResponse>, (StatusCode, Json<ApiError>)> {
    info!("api take-snapshot requested");
    let (snapshot_id, processes_requested) =
        crate::trigger_snapshot(&state).await.map_err(|e| {
            error!(%e, "api take-snapshot failed to trigger snapshot");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, e)
        })?;

    // Read back process statuses for the response
    let db_path = state.db_path.clone();
    tokio::task::spawn_blocking(move || {
        let conn = Connection::open(&*db_path)
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("db open: {e}")))?;

        let mut responded = 0usize;
        let mut timed_out = 0usize;
        let mut error = 0usize;
        let captured_at_ns: i64 = conn
            .query_row(
                "SELECT COALESCE(completed_at_ns, requested_at_ns) FROM snapshots WHERE snapshot_id = ?1",
                params![snapshot_id],
                |row| row.get(0),
            )
            .map_err(|e| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("snapshot time query: {e}"),
                )
            })?;

        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM snapshot_processes WHERE snapshot_id = ?1 GROUP BY status")
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("prepare: {e}")))?;

        let rows = stmt
            .query_map(params![snapshot_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, usize>(1)?))
            })
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

        for row in rows {
            let (status, count) = row.map_err(|e| {
                api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("row: {e}"))
            })?;
            match status.as_str() {
                "responded" => responded += count,
                "timeout" => timed_out += count,
                "error" => error += count,
                _ => {}
            }
        }

        info!(
            snapshot_id,
            requested = processes_requested,
            responded,
            timed_out,
            error,
            captured_at_ns,
            "api take-snapshot completed"
        );
        Ok(Json(TakeSnapshotResponse {
            snapshot_id,
            captured_at_ns,
            requested: processes_requested,
            responded,
            timed_out,
            error,
        }))
    })
    .await
    .map_err(|e| {
        error!(%e, "api take-snapshot response join error");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}"))
    })?
}

// ── GET /api/snapshot-progress ───────────────────────────────────

pub async fn api_snapshot_progress(
    State(state): State<AppState>,
) -> Result<Json<SnapshotProgressResponse>, (StatusCode, Json<ApiError>)> {
    let ctl = state.snapshot_ctl.inner.lock().await;

    if let Some(in_flight) = &ctl.in_flight {
        let pending_processes: Vec<String> = in_flight.pending.iter().cloned().collect();
        let responded_processes: Vec<String> = in_flight
            .requested
            .difference(&in_flight.pending)
            .cloned()
            .collect();
        debug!(
            snapshot_id = in_flight.snapshot_id,
            requested = in_flight.requested.len(),
            responded = responded_processes.len(),
            pending = pending_processes.len(),
            "api snapshot-progress active"
        );

        return Ok(Json(SnapshotProgressResponse {
            active: true,
            snapshot_id: Some(in_flight.snapshot_id),
            requested: in_flight.requested.len(),
            responded: responded_processes.len(),
            pending: in_flight.pending.len(),
            responded_processes,
            pending_processes,
        }));
    }

    debug!("api snapshot-progress inactive");
    Ok(Json(SnapshotProgressResponse {
        active: false,
        snapshot_id: None,
        requested: 0,
        responded: 0,
        pending: 0,
        responded_processes: Vec::new(),
        pending_processes: Vec::new(),
    }))
}

// ── GET /api/connections ─────────────────────────────────────────

pub async fn api_connections(
    State(state): State<AppState>,
) -> Result<Json<ConnectionsResponse>, (StatusCode, Json<ApiError>)> {
    let ctl = state.snapshot_ctl.inner.lock().await;
    let connected_processes = ctl.connections.len();
    let mut processes: Vec<ConnectedProcessInfo> = ctl
        .connections
        .values()
        .map(|conn| ConnectedProcessInfo {
            proc_key: conn.proc_key.clone(),
            process_name: if conn.process_name.is_empty() {
                conn.proc_key.clone()
            } else {
                conn.process_name.clone()
            },
        })
        .collect();
    processes.sort_by(|a, b| {
        a.process_name
            .cmp(&b.process_name)
            .then_with(|| a.proc_key.cmp(&b.proc_key))
    });
    processes.dedup_by(|a, b| a.proc_key == b.proc_key);

    debug!(
        connected_processes,
        in_flight = ctl.in_flight.is_some(),
        "api connections"
    );
    Ok(Json(ConnectionsResponse {
        connected_processes,
        can_take_snapshot: connected_processes > 0 && ctl.in_flight.is_none(),
        processes,
    }))
}

pub async fn api_snapshot_processes(
    State(state): State<AppState>,
    AxumPath(snapshot_id): AxumPath<i64>,
) -> Result<Json<SnapshotProcessesResponse>, (StatusCode, Json<ApiError>)> {
    let db_path = state.db_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = Connection::open(&*db_path)
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("db open: {e}")))?;

        let mut stmt = conn
            .prepare(
                "WITH command_nodes AS (
                 SELECT snapshot_id, proc_key,
                        MAX(json_extract(attrs_json, '$.cmd.program')) AS command,
                        MAX(json_extract(attrs_json, '$.cmd.args_preview')) AS cmd_args_preview
                 FROM nodes
                 WHERE snapshot_id = ?1 AND kind = 'command'
                 GROUP BY snapshot_id, proc_key
                )
                SELECT sp.process, sp.pid, sp.proc_key, sp.status, sp.recv_at_ns, sp.error_text,
                       cn.command, cn.cmd_args_preview
                FROM snapshot_processes AS sp
                LEFT JOIN command_nodes AS cn
                  ON cn.snapshot_id = sp.snapshot_id AND cn.proc_key = sp.proc_key
                WHERE sp.snapshot_id = ?1
                ORDER BY sp.process, sp.proc_key",
            )
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("prepare: {e}")))?;

        let rows = stmt
            .query_map(params![snapshot_id], |row| {
                Ok(SnapshotProcessInfo {
                    process: row.get(0)?,
                    pid: row.get(1)?,
                    proc_key: row.get(2)?,
                    status: row.get(3)?,
                    recv_at_ns: row.get(4)?,
                    error_text: row.get(5)?,
                    command: row.get(6)?,
                    cmd_args_preview: row.get(7)?,
                })
            })
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

        let mut processes = Vec::new();
        for row in rows {
            processes.push(row.map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?);
        }

        Ok(SnapshotProcessesResponse {
            snapshot_id,
            processes,
        })
    })
    .await
    .map_err(|e| {
        error!(snapshot_id = snapshot_id, %e, "api snapshot_processes response join error");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}"))
    })??;

    debug!(
        snapshot_id = snapshot_id,
        process_count = result.processes.len(),
        "api snapshot_processes"
    );
    Ok(Json(result))
}

pub async fn api_process_debug(
    State(state): State<AppState>,
    Json(req): Json<ProcessDebugRequest>,
) -> Result<Json<ProcessDebugResponse>, (StatusCode, Json<ApiError>)> {
    let action = req.action.trim().to_ascii_lowercase();
    if action.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "missing action"));
    }

    let proc_key = req.proc_key.trim().to_string();
    if proc_key.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "missing proc_key"));
    }

    let db_path = state.db_path.clone();
    let snapshot_id = req.snapshot_id;
    let run = req.run;
    let output_store = state.process_debug_results.clone();

    let result = tokio::task::spawn_blocking(move || {
        let conn = Connection::open(&*db_path)
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("db open: {e}")))?;

        let row = conn
            .query_row(
                "SELECT sp.process, sp.pid, sp.proc_key
                 FROM snapshot_processes AS sp
                 WHERE sp.snapshot_id = ?1 AND sp.proc_key = ?2",
                params![snapshot_id, proc_key],
                |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, String>(2)?))
                },
            )
            .optional()
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("query: {e}")))?;

        let Some((process, pid_opt, proc_key)) = row else {
            return Err(api_error(StatusCode::NOT_FOUND, "process not found in snapshot"));
        };

        let Some(pid) = pid_opt else {
            return Ok(ProcessDebugResponse {
                snapshot_id,
                process,
                proc_key,
                pid: None,
                action,
                command: "pid unavailable for this process".to_string(),
                status: "missing_pid".to_string(),
                status_message: Some("No pid available for this snapshot process".to_string()),
                command_output: None,
                command_exit_code: None,
                result_url: None,
            });
        };

        let command = run_debug_command_text(action.as_str(), pid);
        let (status, status_message, output, exit_code) = if cfg!(target_os = "macos") {
            if run {
                match action.as_str() {
                    "sample" => match run_sample_command(pid) {
                        Ok((command_output, exit_code)) => {
                            let status = if exit_code == 0 { "executed" } else { "execution_failed" };
                            (status.to_string(), None, Some(command_output), Some(exit_code))
                        }
                        Err(err) => (
                            "execution_error".to_string(),
                            Some(err),
                            None,
                            None,
                        ),
                    },
                    "spindump" => match run_spindump_command(pid) {
                        Ok((command_output, exit_code)) => {
                            let status = if exit_code == 0 { "executed" } else { "execution_failed" };
                            (status.to_string(), None, Some(command_output), Some(exit_code))
                        }
                        Err(err) => (
                            "execution_error".to_string(),
                            Some(err),
                            None,
                            None,
                        ),
                    },
                    _ => {
                        return Err(api_error(
                            StatusCode::BAD_REQUEST,
                            format!("unsupported action: {action}"),
                        ));
                    }
                }
            } else {
                (
                    "prepared".to_string(),
                    Some(format!("{action} command prepared, not executed")),
                    None,
                    None,
                )
            }
        } else {
            (
                "unsupported".to_string(),
                Some(format!("{action} is available on macOS only")),
                None,
                None,
            )
        };

        let result_url = if run {
            output
                .as_ref()
                .map(|output| cache_process_debug_output(&output_store, output))
                .transpose()?
        } else {
            None
        };

        Ok(ProcessDebugResponse {
            snapshot_id,
            process,
            proc_key,
            pid: Some(pid),
            action,
            command,
            status,
            status_message,
            command_output: output,
            command_exit_code: exit_code,
            result_url,
        })
    })
    .await
    .map_err(|e| {
        error!(snapshot_id = snapshot_id, %e, "api process debug join error");
        api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("join: {e}"))
    })??;

    Ok(Json(result))
}

pub async fn api_process_debug_result(
    State(state): State<AppState>,
    AxumPath(result_id): AxumPath<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiError>)> {
    let output = state
        .process_debug_results
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "debug output cache unavailable"))?
        .get(&result_id)
        .cloned()
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "process debug result not found"))?;

    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        output,
    ))
}

fn run_sample_command(pid: i64) -> Result<(String, i32), String> {
    let pid = pid.to_string();
    run_debug_command("sample", &[pid.as_str(), "1"])
}

fn run_spindump_command(pid: i64) -> Result<(String, i32), String> {
    let pid = pid.to_string();
    run_debug_command("sudo", &["-n", "spindump", pid.as_str()])
}

fn run_debug_command_text(action: &str, pid: i64) -> String {
    match action {
        "sample" => format!("sample {pid} 1"),
        "spindump" => format!("sudo -n spindump {pid}"),
        _ => action.to_string(),
    }
}

fn run_debug_command(binary: &str, args: &[&str]) -> Result<(String, i32), String> {
    let output = Command::new(binary)
        .args(args)
        .output()
        .map_err(|err| format!("failed to execute command: {err}"))?;

    let mut result = String::new();
    if !output.stdout.is_empty() {
        result.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    let status_code = output.status.code().unwrap_or(-1);
    if result.len() > 65_536 {
        result.truncate(65_536);
    }
    Ok((result, status_code))
}

fn next_debug_output_suffix() -> String {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let sequence = PROCESS_DEBUG_OUTPUT_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{suffix}-{sequence}")
}

fn next_debug_result_id() -> String {
    format!("{PROCESS_DEBUG_OUTPUT_PREFIX}-{}", next_debug_output_suffix())
}

fn cache_process_debug_output(
    output_store: &std::sync::Arc<std::sync::Mutex<HashMap<String, String>>>,
    output: &str,
) -> Result<String, (StatusCode, Json<ApiError>)> {
    let result_id = next_debug_result_id();
    output_store
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "debug output cache unavailable"))?
        .insert(result_id.clone(), output.to_string());
    Ok(format!("/api/process-debug-result/{result_id}"))
}

// ── POST /api/sql ────────────────────────────────────────────────

pub async fn api_sql(
    State(state): State<AppState>,
    Json(req): Json<SqlRequest>,
) -> Result<Json<SqlResponse>, (StatusCode, Json<ApiError>)> {
    let snapshot_id = req.snapshot_id;
    let sql_preview = preview_sql(&req.sql);
    let param_count = req.params.len();
    debug!(
        snapshot_id = snapshot_id,
        %sql_preview,
        param_count = param_count,
        "api sql request"
    );
    let db_path = state.db_path.clone();
    let result = tokio::task::spawn_blocking(move || sql_blocking(&db_path, req))
        .await
        .map_err(|e| {
            error!(snapshot_id = snapshot_id, %e, "api sql join error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("join error: {e}"),
            )
        })?;
    match result {
        Ok(resp) => {
            debug!(
                snapshot_id = snapshot_id,
                row_count = resp.row_count,
                truncated = resp.truncated,
                column_count = resp.columns.len(),
                "api sql response"
            );
            Ok(resp)
        }
        Err(err) => {
            warn!(
                snapshot_id = snapshot_id,
                %sql_preview,
                "api sql request rejected or failed"
            );
            Err(err)
        }
    }
}

fn preview_sql(sql: &str) -> String {
    const LIMIT: usize = 160;
    if sql.len() <= LIMIT {
        return sql.to_string();
    }
    let mut out = sql[..LIMIT].to_string();
    out.push_str("...");
    out
}

fn sql_blocking(
    db_path: &PathBuf,
    req: SqlRequest,
) -> Result<Json<SqlResponse>, (StatusCode, Json<ApiError>)> {
    let conn = Connection::open(db_path)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, format!("db open: {e}")))?;

    // 1. Create scoped TEMP VIEWs for this snapshot_id
    create_scoped_views(&conn, req.snapshot_id).map_err(|e| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("create views: {e}"),
        )
    })?;

    // 2. Install authorizer to block direct main.* table access
    install_authorizer(&conn);

    // 3. Install progress handler for execution time limit
    let deadline = Instant::now() + std::time::Duration::from_millis(MAX_EXECUTION_MS);
    install_progress_handler(&conn, deadline);

    // 4. Reject multiple statements and direct main schema access
    let sql = req.sql.trim();
    if sql.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "empty SQL"));
    }
    reject_multiple_statements(sql)?;
    reject_main_schema_access(sql)?;

    // 5. Prepare the statement
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| api_error(StatusCode::BAD_REQUEST, format!("prepare error: {e}")))?;

    // 6. Bind parameters
    let param_values = convert_params(&req.params)?;
    for (i, val) in param_values.iter().enumerate() {
        stmt.raw_bind_parameter(i + 1, val).map_err(|e| {
            api_error(
                StatusCode::BAD_REQUEST,
                format!("bind param {}: {e}", i + 1),
            )
        })?;
    }

    // 7. Execute with row/byte caps
    let column_count = stmt.column_count();
    let columns: Vec<String> = (0..column_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let mut rows: Vec<Vec<JsonValue>> = Vec::new();
    let mut total_bytes: usize = 0;
    let mut truncated = false;

    let mut raw_rows = stmt.raw_query();
    loop {
        let row = match raw_rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(e) => {
                if is_interrupt_error(&e) {
                    truncated = true;
                    break;
                }
                return Err(api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("query error: {e}"),
                ));
            }
        };

        if rows.len() >= MAX_ROWS {
            truncated = true;
            break;
        }

        let mut row_values = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let val = sqlite_value_to_json(row, i);
            row_values.push(val);
        }

        let row_json = serde_json::to_string(&row_values).unwrap_or_default();
        total_bytes += row_json.len();
        if total_bytes > MAX_RESPONSE_BYTES {
            truncated = true;
            break;
        }

        rows.push(row_values);
    }

    let row_count = rows.len();

    Ok(Json(SqlResponse {
        snapshot_id: req.snapshot_id,
        columns,
        rows,
        row_count,
        truncated,
    }))
}

// ── Multiple statement rejection ─────────────────────────────────

/// Reject SQL input that contains more than one statement.
///
/// Scans for a semicolon that is followed by non-whitespace characters,
/// skipping quoted strings and comments.
fn reject_multiple_statements(sql: &str) -> Result<(), (StatusCode, Json<ApiError>)> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut found_semicolon = false;

    while i < len {
        match bytes[i] {
            // Skip single-quoted strings
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            i += 1; // escaped quote
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            // Skip double-quoted identifiers
            b'"' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'"' {
                        i += 1;
                        if i < len && bytes[i] == b'"' {
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            // Skip -- line comments
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
            }
            // Skip /* block comments */
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b';' => {
                found_semicolon = true;
                i += 1;
            }
            _ => {
                if found_semicolon && !bytes[i].is_ascii_whitespace() {
                    return Err(api_error(
                        StatusCode::BAD_REQUEST,
                        "multiple statements not allowed",
                    ));
                }
                i += 1;
            }
        }
    }
    Ok(())
}

/// Reject queries that try to bypass scoped TEMP VIEWs by referencing `main.*` directly.
fn reject_main_schema_access(sql: &str) -> Result<(), (StatusCode, Json<ApiError>)> {
    // Case-insensitive check for `main.` prefix on scoped table names.
    let lower = sql.to_ascii_lowercase();
    for (table, _) in SCOPED_TABLES {
        if lower.contains(&format!("main.{table}")) || lower.contains(&format!("main.[{table}]")) {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                format!("direct access to main.{table} is not allowed; use {table} instead"),
            ));
        }
    }
    Ok(())
}

// ── Scoped TEMP VIEWs ───────────────────────────────────────────

fn create_scoped_views(conn: &Connection, snapshot_id: i64) -> rusqlite::Result<()> {
    for (table, cols) in SCOPED_TABLES {
        conn.execute_batch(&format!(
            "CREATE TEMP VIEW [{table}] AS SELECT {cols} FROM main.[{table}] WHERE snapshot_id = {snapshot_id}"
        ))?;
    }
    Ok(())
}

// ── SQLite authorizer ────────────────────────────────────────────

fn install_authorizer(conn: &Connection) {
    conn.authorizer(Some(|ctx: rusqlite::hooks::AuthContext<'_>| {
        use rusqlite::hooks::{AuthAction, Authorization};

        match ctx.action {
            AuthAction::Read { table_name, .. } => {
                // Block sqlite_master reads
                if table_name == "sqlite_master" || table_name == "sqlite_temp_master" {
                    return Authorization::Deny;
                }

                // Allow all column reads — scoped TEMP VIEWs shadow the main
                // table names so user queries go through the view. We block
                // direct `main.*` access via reject_main_schema_access().
                Authorization::Allow
            }
            AuthAction::Select => Authorization::Allow,
            AuthAction::Function { .. } => Authorization::Allow,
            AuthAction::Recursive => Authorization::Allow,
            // Block everything else
            _ => Authorization::Deny,
        }
    }));
}

// ── Progress handler (execution time limit) ──────────────────────

fn install_progress_handler(conn: &Connection, deadline: Instant) {
    conn.progress_handler(
        PROGRESS_HANDLER_OPS,
        Some(move || Instant::now() > deadline),
    );
}

fn is_interrupt_error(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::OperationInterrupted,
                ..
            },
            _
        )
    )
}

// ── Parameter conversion ─────────────────────────────────────────

fn convert_params(params: &[JsonValue]) -> Result<Vec<Value>, (StatusCode, Json<ApiError>)> {
    params
        .iter()
        .enumerate()
        .map(|(i, v)| match v {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(b) => Ok(Value::Integer(if *b { 1 } else { 0 })),
            JsonValue::Number(n) => {
                if let Some(int) = n.as_i64() {
                    Ok(Value::Integer(int))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Real(f))
                } else {
                    Err(api_error(
                        StatusCode::BAD_REQUEST,
                        format!("param {}: unsupported number", i + 1),
                    ))
                }
            }
            JsonValue::String(s) => Ok(Value::Text(s.clone())),
            _ => Err(api_error(
                StatusCode::BAD_REQUEST,
                format!("param {}: unsupported type (object/array)", i + 1),
            )),
        })
        .collect()
}

// ── SQLite value → JSON ──────────────────────────────────────────

fn sqlite_value_to_json(row: &rusqlite::Row<'_>, idx: usize) -> JsonValue {
    use rusqlite::types::ValueRef;

    match row.get_ref(idx) {
        Ok(ValueRef::Null) => JsonValue::Null,
        Ok(ValueRef::Integer(i)) => JsonValue::Number(i.into()),
        Ok(ValueRef::Real(f)) => serde_json::Number::from_f64(f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        Ok(ValueRef::Text(bytes)) => {
            let s = String::from_utf8_lossy(bytes);
            JsonValue::String(s.into_owned())
        }
        Ok(ValueRef::Blob(_)) => JsonValue::Null,
        Err(_) => JsonValue::Null,
    }
}
