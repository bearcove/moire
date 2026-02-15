mod projection;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use peeps_types::{Direction, ProcessDump};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone)]
struct AppState {
    db_path: Arc<PathBuf>,
    seq: Arc<AtomicU64>,
}

#[derive(Debug, Serialize)]
struct SnapshotMeta {
    seq: u64,
}

#[derive(Debug, Serialize)]
struct NodeRow {
    id: String,
    kind: String,
    process: String,
    attrs_json: String,
}

#[derive(Debug, Serialize)]
struct EdgeRow {
    src_id: String,
    dst_id: String,
    kind: String,
    attrs_json: String,
}

#[derive(Debug, Serialize)]
struct GraphResponse {
    seq: u64,
    nodes: Vec<NodeRow>,
    edges: Vec<EdgeRow>,
}

#[derive(Debug, Deserialize)]
struct StuckQuery {
    min_secs: Option<f64>,
}

#[tokio::main]
async fn main() {
    let tcp_addr = std::env::var("PEEPS_LISTEN").unwrap_or_else(|_| "127.0.0.1:9119".into());
    let http_addr = std::env::var("PEEPS_HTTP").unwrap_or_else(|_| "127.0.0.1:9130".into());
    let db_path = std::env::var("PEEPS_DB").unwrap_or_else(|_| "./peeps-web.sqlite".into());

    init_db(&db_path).expect("init sqlite schema");

    let state = AppState {
        db_path: Arc::new(PathBuf::from(&db_path)),
        seq: Arc::new(AtomicU64::new(0)),
    };

    let tcp_listener = TcpListener::bind(&tcp_addr)
        .await
        .unwrap_or_else(|e| panic!("[peeps-web] failed to bind TCP on {tcp_addr}: {e}"));
    eprintln!("[peeps-web] TCP listener on {tcp_addr} (ProcessDump ingest)");

    let http_listener = TcpListener::bind(&http_addr)
        .await
        .unwrap_or_else(|e| panic!("[peeps-web] failed to bind HTTP on {http_addr}: {e}"));
    eprintln!("[peeps-web] HTTP server on http://{http_addr}/");
    eprintln!("[peeps-web] sqlite DB: {db_path}");

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/snapshots", get(api_snapshots))
        .route("/api/snapshot/latest", get(api_snapshot_latest))
        .route("/api/snapshot/:seq/graph", get(api_snapshot_graph))
        .route("/api/snapshot/:seq/stuck-requests", get(api_snapshot_stuck_requests))
        .with_state(state.clone());

    tokio::select! {
        _ = run_tcp_acceptor(tcp_listener, state.clone()) => {}
        result = axum::serve(http_listener, app) => {
            if let Err(e) = result {
                eprintln!("[peeps-web] HTTP server error: {e}");
            }
        }
    }
}

async fn health() -> impl IntoResponse {
    "ok"
}

async fn api_snapshots(State(state): State<AppState>) -> Json<Vec<SnapshotMeta>> {
    let conn = open_db(&state.db_path);
    let mut stmt = conn
        .prepare("SELECT DISTINCT seq FROM nodes ORDER BY seq DESC LIMIT 200")
        .unwrap();
    let rows = stmt
        .query_map([], |row| Ok(SnapshotMeta { seq: row.get(0)? }))
        .unwrap();
    let out = rows.filter_map(Result::ok).collect::<Vec<_>>();
    Json(out)
}

async fn api_snapshot_latest(State(state): State<AppState>) -> Json<serde_json::Value> {
    let conn = open_db(&state.db_path);
    let seq: Option<u64> = conn
        .query_row("SELECT MAX(seq) FROM nodes", [], |row| row.get(0))
        .ok();
    Json(json!({ "seq": seq.unwrap_or(0) }))
}

async fn api_snapshot_graph(
    State(state): State<AppState>,
    Path(seq): Path<u64>,
) -> Json<GraphResponse> {
    let conn = open_db(&state.db_path);

    let mut ns = conn
        .prepare("SELECT id, kind, process, attrs_json FROM nodes WHERE seq = ?1")
        .unwrap();
    let nodes = ns
        .query_map([seq], |row| {
            Ok(NodeRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                process: row.get(2)?,
                attrs_json: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut es = conn
        .prepare("SELECT src_id, dst_id, kind, attrs_json FROM edges WHERE seq = ?1")
        .unwrap();
    let edges = es
        .query_map([seq], |row| {
            Ok(EdgeRow {
                src_id: row.get(0)?,
                dst_id: row.get(1)?,
                kind: row.get(2)?,
                attrs_json: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    Json(GraphResponse { seq, nodes, edges })
}

async fn api_snapshot_stuck_requests(
    State(state): State<AppState>,
    Path(seq): Path<u64>,
    Query(q): Query<StuckQuery>,
) -> Json<Vec<NodeRow>> {
    let min_secs = q.min_secs.unwrap_or(5.0);
    let conn = open_db(&state.db_path);
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, process, attrs_json
             FROM nodes
             WHERE seq = ?1
               AND kind = 'request'
               AND CAST(json_extract(attrs_json, '$.elapsed_secs') AS REAL) >= ?2
             ORDER BY CAST(json_extract(attrs_json, '$.elapsed_secs') AS REAL) DESC",
        )
        .unwrap();
    let rows = stmt
        .query_map(params![seq, min_secs], |row| {
            Ok(NodeRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                process: row.get(2)?,
                attrs_json: row.get(3)?,
            })
        })
        .unwrap();
    Json(rows.filter_map(Result::ok).collect())
}

fn init_db(path: &str) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;

        CREATE TABLE IF NOT EXISTS nodes (
            seq        INTEGER NOT NULL,
            id         TEXT    NOT NULL,
            kind       TEXT    NOT NULL,
            process    TEXT    NOT NULL,
            attrs_json TEXT    NOT NULL,
            PRIMARY KEY (seq, id)
        );

        CREATE TABLE IF NOT EXISTS edges (
            seq        INTEGER NOT NULL,
            src_id     TEXT    NOT NULL,
            dst_id     TEXT    NOT NULL,
            kind       TEXT    NOT NULL,
            attrs_json TEXT    NOT NULL,
            PRIMARY KEY (seq, src_id, dst_id, kind)
        );

        CREATE INDEX IF NOT EXISTS idx_nodes_seq_kind ON nodes(seq, kind);
        CREATE INDEX IF NOT EXISTS idx_nodes_seq_process ON nodes(seq, process);
        CREATE INDEX IF NOT EXISTS idx_edges_seq_src ON edges(seq, src_id);
        CREATE INDEX IF NOT EXISTS idx_edges_seq_dst ON edges(seq, dst_id);
        CREATE INDEX IF NOT EXISTS idx_edges_seq_kind ON edges(seq, kind);
        ",
    )?;
    Ok(())
}

fn open_db(path: &PathBuf) -> Connection {
    Connection::open(path).expect("open sqlite")
}

async fn run_tcp_acceptor(listener: TcpListener, state: AppState) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                eprintln!("[peeps-web] TCP connection from {addr}");
                let st = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, st).await {
                        eprintln!("[peeps-web] connection error: {e}");
                    }
                });
            }
            Err(e) => {
                eprintln!("[peeps-web] accept failed: {e}");
            }
        }
    }
}

// Ingest wire format (v1):
// [u32 big-endian frame_len][UTF-8 JSON ProcessDump]
async fn handle_conn(mut stream: TcpStream, state: AppState) -> Result<(), String> {
    loop {
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| format!("read frame len failed: {e}"))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 128 * 1024 * 1024 {
            return Err(format!("frame too large: {len} bytes"));
        }
        let mut frame = vec![0u8; len];
        stream
            .read_exact(&mut frame)
            .await
            .map_err(|e| format!("read frame payload failed: {e}"))?;

        let json = std::str::from_utf8(&frame).map_err(|e| format!("utf8 decode failed: {e}"))?;
        let dump = facet_json::from_str::<ProcessDump>(json)
            .map_err(|e| format!("json decode ProcessDump failed: {e}"))?;

        let seq = state.seq.fetch_add(1, Ordering::Relaxed) + 1;
        persist_dump(&state.db_path, seq, &dump)?;

        eprintln!(
            "[peeps-web] dump from {} (pid {}): {} tasks, {} threads => seq {}",
            dump.process_name,
            dump.pid,
            dump.tasks.len(),
            dump.threads.len(),
            seq
        );
    }
}

fn persist_dump(db_path: &PathBuf, seq: u64, dump: &ProcessDump) -> Result<(), String> {
    let mut conn = open_db(db_path);
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let process_id = format!("process:{}:{}", dump.process_name, dump.pid);
    tx.execute(
        "INSERT OR REPLACE INTO nodes(seq,id,kind,process,attrs_json) VALUES (?1,?2,'process',?3,?4)",
        params![
            seq,
            process_id,
            dump.process_name,
            json!({"pid": dump.pid, "timestamp": dump.timestamp}).to_string()
        ],
    )
    .map_err(|e| e.to_string())?;

    let mut request_ids = BTreeSet::new();

    for t in &dump.tasks {
        let task_id = format!("task:{}:{}:{}", dump.process_name, dump.pid, t.id);
        tx.execute(
            "INSERT OR REPLACE INTO nodes(seq,id,kind,process,attrs_json) VALUES (?1,?2,'task',?3,?4)",
            params![
                seq,
                task_id,
                dump.process_name,
                json!({
                    "task_id": t.id,
                    "name": t.name,
                    "state": format!("{:?}", t.state),
                    "age_secs": t.age_secs,
                    "parent_task_id": t.parent_task_id,
                })
                .to_string()
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT OR REPLACE INTO edges(seq,src_id,dst_id,kind,attrs_json) VALUES (?1,?2,?3,'task_in_process',?4)",
            params![seq, task_id, process_id, "{}"],
        )
        .map_err(|e| e.to_string())?;
    }

    for w in &dump.future_waits {
        let future_id = format!("future:{}:{}:{}", dump.process_name, dump.pid, w.future_id);
        let task_id = format!("task:{}:{}:{}", dump.process_name, dump.pid, w.task_id);

        tx.execute(
            "INSERT OR REPLACE INTO nodes(seq,id,kind,process,attrs_json) VALUES (?1,?2,'future',?3,?4)",
            params![
                seq,
                future_id,
                dump.process_name,
                json!({
                    "future_id": w.future_id,
                    "resource": w.resource,
                    "total_pending_secs": w.total_pending_secs,
                    "pending_count": w.pending_count,
                    "ready_count": w.ready_count,
                })
                .to_string()
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT OR REPLACE INTO edges(seq,src_id,dst_id,kind,attrs_json) VALUES (?1,?2,?3,'task_awaits_future',?4)",
            params![
                seq,
                task_id,
                future_id,
                json!({"wait_secs": w.total_pending_secs}).to_string()
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    if let Some(roam) = &dump.roam {
        for c in &roam.connections {
            for r in &c.in_flight {
                let request_id = format!(
                    "request:{}:{}:{}:{}",
                    dump.process_name, dump.pid, c.name, r.request_id
                );
                request_ids.insert(request_id.clone());

                tx.execute(
                    "INSERT OR REPLACE INTO nodes(seq,id,kind,process,attrs_json) VALUES (?1,?2,'request',?3,?4)",
                    params![
                        seq,
                        request_id,
                        dump.process_name,
                        json!({
                            "request_id": r.request_id,
                            "connection": c.name,
                            "method": r.method_name,
                            "direction": match r.direction { Direction::Incoming => "incoming", Direction::Outgoing => "outgoing" },
                            "elapsed_secs": r.elapsed_secs,
                            "task_id": r.task_id,
                            "task_name": r.task_name,
                        })
                        .to_string()
                    ],
                )
                .map_err(|e| e.to_string())?;

                if let Some(task_id_num) = r.task_id {
                    let task_id = format!("task:{}:{}:{}", dump.process_name, dump.pid, task_id_num);
                    tx.execute(
                        "INSERT OR REPLACE INTO edges(seq,src_id,dst_id,kind,attrs_json) VALUES (?1,?2,?3,'request_handled_by_task',?4)",
                        params![seq, request_id, task_id, "{}"],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    for p in &dump.request_parents {
        let child = format!(
            "request:{}:{}:{}:{}",
            p.child_process, dump.pid, p.child_connection, p.child_request_id
        );
        let parent = format!(
            "request:{}:{}:{}:{}",
            p.parent_process, dump.pid, p.parent_connection, p.parent_request_id
        );
        if request_ids.contains(&child) {
            tx.execute(
                "INSERT OR REPLACE INTO edges(seq,src_id,dst_id,kind,attrs_json) VALUES (?1,?2,?3,'request_parent',?4)",
                params![seq, child, parent, "{}"],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())
}
