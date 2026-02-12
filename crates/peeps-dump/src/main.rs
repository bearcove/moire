//! peeps CLI tool — live dashboard server
//!
//! Commands:
//! - `peeps` — Start dashboard (TCP on :9119, HTTP on :9120)
//! - `peeps clean` — Clean stale dump files

mod http;
mod server;

use std::sync::Arc;

use server::DashboardState;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "clean" {
        peeps::clean_dumps();
        eprintln!("[peeps] Cleaned dump directory");
        return;
    }

    let state = Arc::new(DashboardState::new());

    // Load existing file-based dumps for backwards compatibility.
    let file_dumps = peeps::read_all_dumps();
    if !file_dumps.is_empty() {
        eprintln!(
            "[peeps] Loaded {} existing dumps from {}",
            file_dumps.len(),
            peeps::DUMP_DIR
        );
        for dump in file_dumps {
            eprintln!(
                "  {} (pid {}): {} tasks, {} threads",
                dump.process_name,
                dump.pid,
                dump.tasks.len(),
                dump.threads.len()
            );
            state.upsert_dump(dump).await;
        }
    }

    let tcp_addr = std::env::var("PEEPS_LISTEN").unwrap_or_else(|_| "127.0.0.1:9119".into());
    let http_addr = std::env::var("PEEPS_HTTP").unwrap_or_else(|_| "127.0.0.1:9120".into());

    let tcp_listener = tokio::net::TcpListener::bind(&tcp_addr)
        .await
        .unwrap_or_else(|e| panic!("[peeps] failed to bind TCP on {tcp_addr}: {e}"));
    eprintln!("[peeps] TCP listener on {tcp_addr} (instrumented processes connect here)");

    let http_listener = tokio::net::TcpListener::bind(&http_addr)
        .await
        .unwrap_or_else(|e| panic!("[peeps] failed to bind HTTP on {http_addr}: {e}"));
    eprintln!("[peeps] HTTP server on http://{http_addr}/");

    let app = http::router(Arc::clone(&state));

    tokio::select! {
        _ = server::run_tcp_acceptor(tcp_listener, Arc::clone(&state)) => {}
        result = axum::serve(http_listener, app) => {
            if let Err(e) = result {
                eprintln!("[peeps] HTTP server error: {e}");
            }
        }
    }
}
