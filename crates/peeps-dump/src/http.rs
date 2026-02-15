use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use peeps_types::ProcessDump;
use rust_embed::Embed;

use crate::server::DashboardState;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

pub fn router(state: Arc<DashboardState>) -> Router {
    Router::new()
        .route("/api/dumps", get(api_dumps))
        .route("/api/summary", get(api_summary))
        .route("/api/problems", get(api_problems))
        .route("/api/deadlocks", get(api_deadlocks))
        .route("/api/tasks", get(api_tasks))
        .route("/api/threads", get(api_threads))
        .route("/api/locks", get(api_locks))
        .route("/api/sync", get(api_sync))
        .route("/api/requests", get(api_requests))
        .route("/api/connections", get(api_connections))
        .route("/api/processes", get(api_processes))
        .route("/api/shm", get(api_shm))
        .route("/api/ws", get(ws_upgrade))
        .fallback(static_handler)
        .with_state(state)
}

// ── Response envelope ────────────────────────────────────────────

fn json_envelope_response(seq: u64, data_json: &str) -> Response {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let envelope = format!(
        r#"{{"version":1,"seq":{seq},"server_time_ms":{now_ms},"data":{data_json}}}"#
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        envelope,
    )
        .into_response()
}

fn serialization_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("serialization error: {e}"),
    )
        .into_response()
}

// ── Endpoints ────────────────────────────────────────────────────

async fn api_dumps(State(state): State<Arc<DashboardState>>) -> Response {
    let seq = state.current_seq();
    let payload = state.dashboard_payload().await;
    match facet_json::to_string(&payload) {
        Ok(json) => json_envelope_response(seq, &json),
        Err(e) => serialization_error(e),
    }
}

async fn api_summary(State(state): State<Arc<DashboardState>>) -> Response {
    let seq = state.current_seq();
    let dumps = state.all_dumps().await;
    let process_count = dumps.len();
    let task_count: usize = dumps.iter().map(|d| d.tasks.len()).sum();
    let thread_count: usize = dumps.iter().map(|d| d.threads.len()).sum();
    let data = format!(
        r#"{{"process_count":{process_count},"task_count":{task_count},"thread_count":{thread_count},"seq":{seq}}}"#
    );
    json_envelope_response(seq, &data)
}

async fn api_problems(State(state): State<Arc<DashboardState>>) -> Response {
    // For now, same as /api/dumps — client does problem detection.
    api_dumps(State(state)).await
}

async fn api_deadlocks(State(state): State<Arc<DashboardState>>) -> Response {
    let seq = state.current_seq();
    let payload = state.dashboard_payload().await;
    match facet_json::to_string(&payload.deadlock_candidates) {
        Ok(json) => json_envelope_response(seq, &json),
        Err(e) => serialization_error(e),
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

async fn api_connections(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_connections).await
}

async fn api_processes(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_processes).await
}

async fn api_shm(State(state): State<Arc<DashboardState>>) -> Response {
    api_slice(state, slim_for_shm).await
}

async fn api_slice(
    state: Arc<DashboardState>,
    slim: fn(ProcessDump) -> ProcessDump,
) -> Response {
    let seq = state.current_seq();
    let dumps = state
        .all_dumps()
        .await
        .into_iter()
        .map(slim)
        .collect::<Vec<_>>();
    match facet_json::to_string(&dumps) {
        Ok(json) => json_envelope_response(seq, &json),
        Err(e) => serialization_error(e),
    }
}

// ── Slim helpers ─────────────────────────────────────────────────

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

fn slim_for_connections(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.locks = None;
    d.sync = None;
    d.shm = None;
    // Keep roam for connections + channels.
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

fn slim_for_shm(mut d: ProcessDump) -> ProcessDump {
    d.tasks.clear();
    d.wake_edges.clear();
    d.future_wake_edges.clear();
    d.future_waits.clear();
    d.threads.clear();
    d.locks = None;
    d.sync = None;
    d.roam = None;
    // Keep shm.
    d
}

// ── WebSocket (notify-only) ──────────────────────────────────────

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashboardState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<DashboardState>) {
    let now_ms = server_time_ms();
    let latest_seq = state.current_seq();
    let hello = format!(
        r#"{{"type":"hello","version":1,"server_time_ms":{now_ms},"latest_seq":{latest_seq}}}"#
    );
    if socket.send(Message::Text(hello.into())).await.is_err() {
        return;
    }

    let mut rx = state.subscribe();

    loop {
        match rx.recv().await {
            Ok(notif) => {
                let now_ms = server_time_ms();
                let changed_json = notif
                    .changed
                    .iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(",");
                let msg = format!(
                    r#"{{"type":"updated","seq":{},"server_time_ms":{now_ms},"changed":[{changed_json}]}}"#,
                    notif.seq
                );
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                // Lagged — just send the latest seq so the client knows to refresh.
                let now_ms = server_time_ms();
                let seq = state.current_seq();
                let msg = format!(
                    r#"{{"type":"updated","seq":{seq},"server_time_ms":{now_ms},"changed":[]}}"#
                );
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

fn server_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::test_dump;

    #[test]
    fn slim_for_tasks_clears_non_task_fields() {
        let d = slim_for_tasks(test_dump("app", 1));
        // tasks and related edges preserved
        assert!(!d.tasks.is_empty());
        assert!(!d.wake_edges.is_empty());
        assert!(!d.future_wake_edges.is_empty());
        assert!(!d.future_waits.is_empty());
        // everything else cleared
        assert!(d.threads.is_empty());
        assert!(d.locks.is_none());
        assert!(d.sync.is_none());
        assert!(d.roam.is_none());
        assert!(d.shm.is_none());
        // identity preserved
        assert_eq!(d.process_name, "app");
        assert_eq!(d.pid, 1);
    }

    #[test]
    fn slim_for_threads_clears_non_thread_fields() {
        let d = slim_for_threads(test_dump("app", 1));
        // threads preserved
        assert!(!d.threads.is_empty());
        // task-related fields cleared
        assert!(d.tasks.is_empty());
        assert!(d.wake_edges.is_empty());
        assert!(d.future_wake_edges.is_empty());
        assert!(d.future_waits.is_empty());
        // other sections cleared
        assert!(d.locks.is_none());
        assert!(d.sync.is_none());
        assert!(d.roam.is_none());
        assert!(d.shm.is_none());
    }

    #[test]
    fn slim_for_locks_clears_non_lock_fields() {
        let d = slim_for_locks(test_dump("app", 1));
        // locks preserved
        assert!(d.locks.is_some());
        // everything else cleared
        assert!(d.tasks.is_empty());
        assert!(d.wake_edges.is_empty());
        assert!(d.future_wake_edges.is_empty());
        assert!(d.future_waits.is_empty());
        assert!(d.threads.is_empty());
        assert!(d.sync.is_none());
        assert!(d.roam.is_none());
        assert!(d.shm.is_none());
    }

    #[test]
    fn slim_for_sync_clears_non_sync_fields() {
        let d = slim_for_sync(test_dump("app", 1));
        // sync preserved
        assert!(d.sync.is_some());
        // roam kept for channel details
        assert!(d.roam.is_some());
        // everything else cleared
        assert!(d.tasks.is_empty());
        assert!(d.wake_edges.is_empty());
        assert!(d.future_wake_edges.is_empty());
        assert!(d.future_waits.is_empty());
        assert!(d.threads.is_empty());
        assert!(d.locks.is_none());
        assert!(d.shm.is_none());
    }

    #[test]
    fn slim_for_requests_keeps_roam() {
        let d = slim_for_requests(test_dump("app", 1));
        assert!(d.roam.is_some());
        assert!(d.tasks.is_empty());
        assert!(d.threads.is_empty());
        assert!(d.locks.is_none());
        assert!(d.sync.is_none());
        assert!(d.shm.is_none());
    }

    #[test]
    fn slim_for_processes_strips_everything() {
        let d = slim_for_processes(test_dump("app", 1));
        assert!(d.tasks.is_empty());
        assert!(d.wake_edges.is_empty());
        assert!(d.future_wake_edges.is_empty());
        assert!(d.future_waits.is_empty());
        assert!(d.threads.is_empty());
        assert!(d.locks.is_none());
        assert!(d.sync.is_none());
        assert!(d.roam.is_none());
        assert!(d.shm.is_none());
        // identity preserved
        assert_eq!(d.process_name, "app");
        assert_eq!(d.pid, 1);
    }
}

// ── Static file serving ──────────────────────────────────────────

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
