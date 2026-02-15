use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::io::Write;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use flate2::Compression;
use flate2::write::GzEncoder;
use peeps_types::ProcessDump;
use rust_embed::Embed;

use crate::server::DashboardState;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

const WS_CHUNK_START_PREFIX: &str = "__peeps_chunk_start__:";
const WS_CHUNK_PART_PREFIX: &str = "__peeps_chunk_part__:";
const WS_CHUNK_END_PREFIX: &str = "__peeps_chunk_end__:";
const WS_GZIP_PREFIX: &str = "__peeps_gzip_base64__:";
const DEFAULT_WS_CHUNK_BYTES: usize = 256 * 1024;
static WS_CHUNK_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);

pub fn router(state: Arc<DashboardState>) -> Router {
    Router::new()
        .route("/api/dumps", get(api_dumps))
        .route("/api/tasks", get(api_tasks))
        .route("/api/threads", get(api_threads))
        .route("/api/locks", get(api_locks))
        .route("/api/sync", get(api_sync))
        .route("/api/requests", get(api_requests))
        .route("/api/processes", get(api_processes))
        .route("/api/ws", get(ws_upgrade))
        .fallback(static_handler)
        .with_state(state)
}

async fn api_dumps(State(state): State<Arc<DashboardState>>) -> Response {
    let payload = state.dashboard_payload().await;
    match facet_json::to_string(&payload) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialization error: {e}"),
        )
            .into_response(),
    }
}

async fn api_tasks(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_tasks).await
}

async fn api_threads(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_threads).await
}

async fn api_locks(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_locks).await
}

async fn api_sync(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_sync).await
}

async fn api_requests(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_requests).await
}

async fn api_processes(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_processes).await
}

async fn api_slice(
    state: Arc<DashboardState>,
    slim: fn(ProcessDump) -> ProcessDump,
) -> Response {
    let dumps = state
        .all_dumps()
        .await
        .into_iter()
        .map(slim)
        .collect::<Vec<_>>();
    match facet_json::to_string(&dumps) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialization error: {e}"),
        )
            .into_response(),
    }
}

fn slim_for_tasks(mut d: ProcessDump) -> ProcessDump {
    d.threads.clear();
    d.locks = None;
    d.sync = None;
    d.roam = None;
    d.shm = None;
    d
}

fn slim_for_threads(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.locks = None;
    d.sync = None;
    d.roam = None;
    d.shm = None;
    d
}

fn slim_for_locks(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.sync = None;
    d.roam = None;
    d.shm = None;
    d
}

fn slim_for_sync(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.locks = None;
    // Keep roam for channel details used by Sync tab.
    d.shm = None;
    d
}

fn slim_for_requests(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.locks = None;
    d.sync = None;
    d.shm = None;
    d
}

fn slim_for_processes(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.locks = None;
    d.sync = None;
    d.roam = None;
    d.shm = None;
    d
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashboardState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<DashboardState>) {
    // Send initial state immediately
    if let Err(_) = send_dumps(&mut socket, &state).await {
        return;
    }

    let mut rx = state.subscribe();

    loop {
        // Wait for a broadcast notification (new dump arrived)
        match rx.recv().await {
            Ok(()) => {
                if let Err(_) = send_dumps(&mut socket, &state).await {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[peeps] WebSocket subscriber lagged by {n} messages, sending latest");
                if let Err(_) = send_dumps(&mut socket, &state).await {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

async fn send_dumps(
    socket: &mut WebSocket,
    state: &DashboardState,
) -> Result<(), axum::Error> {
    let payload = state.dashboard_payload().await;
    match facet_json::to_string(&payload) {
        Ok(json) => {
            let text = encode_ws_payload(&json);
            let chunk_bytes = ws_chunk_bytes_from_env();
            if text.len() <= chunk_bytes {
                socket.send(Message::Text(text.into())).await
            } else {
                send_chunked_text(socket, &text, chunk_bytes).await
            }
        }
        Err(e) => {
            eprintln!("[peeps] WebSocket serialization error: {e}");
            Ok(())
        }
    }
}

async fn send_chunked_text(
    socket: &mut WebSocket,
    text: &str,
    chunk_bytes: usize,
) -> Result<(), axum::Error> {
    let message_id = WS_CHUNK_MESSAGE_ID.fetch_add(1, Ordering::Relaxed);
    let chunks = split_utf8_chunks(text, chunk_bytes.max(1024));

    eprintln!(
        "[peeps] WebSocket payload too large ({} bytes), streaming in {} chunks",
        text.len(),
        chunks.len()
    );

    socket
        .send(Message::Text(
            format!("{WS_CHUNK_START_PREFIX}{message_id}").into(),
        ))
        .await?;

    for (index, chunk) in chunks.iter().enumerate() {
        let mut message =
            String::with_capacity(WS_CHUNK_PART_PREFIX.len() + 32 + chunk.len());
        message.push_str(WS_CHUNK_PART_PREFIX);
        message.push_str(&message_id.to_string());
        message.push(':');
        message.push_str(&index.to_string());
        message.push(':');
        message.push_str(chunk);
        socket.send(Message::Text(message.into())).await?;
    }

    socket
        .send(Message::Text(
            format!("{WS_CHUNK_END_PREFIX}{message_id}").into(),
        ))
        .await?;

    Ok(())
}

fn split_utf8_chunks(text: &str, max_bytes: usize) -> Vec<&str> {
    if text.is_empty() {
        return vec![""];
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut acc = 0usize;

    for (idx, ch) in text.char_indices() {
        let len = ch.len_utf8();
        if acc + len > max_bytes && idx > start {
            chunks.push(&text[start..idx]);
            start = idx;
            acc = 0;
        }
        acc += len;
    }

    if start < text.len() {
        chunks.push(&text[start..]);
    }
    chunks
}

fn ws_chunk_bytes_from_env() -> usize {
    std::env::var("PEEPS_WS_CHUNK_BYTES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n >= 1024)
        .unwrap_or(DEFAULT_WS_CHUNK_BYTES)
}

fn encode_ws_payload(json: &str) -> String {
    if !ws_gzip_enabled_from_env() {
        return json.to_string();
    }

    match gzip_bytes(json.as_bytes()) {
        Ok(gz) => {
            let b64 = BASE64_STANDARD.encode(gz);
            let mut out = String::with_capacity(WS_GZIP_PREFIX.len() + b64.len());
            out.push_str(WS_GZIP_PREFIX);
            out.push_str(&b64);
            out
        }
        Err(e) => {
            eprintln!("[peeps] WebSocket gzip error: {e}");
            json.to_string()
        }
    }
}

fn gzip_bytes(input: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(input)?;
    encoder.finish()
}

fn ws_gzip_enabled_from_env() -> bool {
    match std::env::var("PEEPS_WS_GZIP") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first
    if !path.is_empty() {
        if let Some(file) = FrontendAssets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data,
            )
                .into_response();
        }
    }

    // SPA fallback: serve index.html for unknown paths
    match FrontendAssets::get("index.html") {
        Some(file) => Html(file.data).into_response(),
        None => (StatusCode::NOT_FOUND, "frontend not built").into_response(),
    }
}
