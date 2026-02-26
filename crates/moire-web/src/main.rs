use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use facet::Facet;
use figue as args;
use moire_types::{CutStatusResponse, QueryRequest, SqlRequest, TriggerCutResponse};
use moire_web::app::{AppState, DevProxyState, build_router};
use moire_web::db::{Db, init_sqlite, load_next_connection_id};
use moire_web::mcp::run_mcp_server;
use moire_web::proxy::{DEFAULT_VITE_ADDR, start_vite_dev_server};
use moire_web::tcp::run_tcp_acceptor;
use tokio::net::TcpListener;
use tracing::{error, info};

#[derive(Facet, Debug)]
struct ServerCli {
    #[facet(flatten)]
    builtins: args::FigueBuiltins,
    #[facet(args::named, default)]
    dev: bool,
}

#[derive(Facet, Debug)]
struct ClientCli {
    #[facet(flatten)]
    builtins: args::FigueBuiltins,
    #[facet(args::subcommand)]
    command: ClientCommand,
}

#[derive(Facet, Debug)]
#[repr(u8)]
enum ClientCommand {
    Cut {
        #[facet(args::named, default)]
        url: Option<String>,
        #[facet(args::named, default)]
        poll_ms: Option<u64>,
        #[facet(args::named, default)]
        timeout_ms: Option<u64>,
    },
    Sql {
        #[facet(args::named, default)]
        url: Option<String>,
        #[facet(args::named)]
        query: String,
    },
    Query {
        #[facet(args::named, default)]
        url: Option<String>,
        #[facet(args::named)]
        name: String,
        #[facet(args::named, default)]
        limit: Option<u32>,
    },
    Snapshot {
        #[facet(args::named, default)]
        url: Option<String>,
    },
}

const REAPER_PIPE_FD_ENV: &str = "MOIRE_REAPER_PIPE_FD";
const REAPER_PGID_ENV: &str = "MOIRE_REAPER_PGID";
const FRONTEND_DIST_ENV: &str = "MOIRE_FRONTEND_DIST";

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:9130";
const DEFAULT_POLL_MS: u64 = 100;
const DEFAULT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_QUERY_LIMIT: u32 = 50;

fn main() {
    // Reaper mode: watch the pipe, kill the process group when it closes.
    // Must NOT call die_with_parent() — we need to outlive the parent briefly.
    #[cfg(unix)]
    if let (Ok(fd_str), Ok(pgid_str)) = (
        std::env::var(REAPER_PIPE_FD_ENV),
        std::env::var(REAPER_PGID_ENV),
    ) && let (Ok(fd), Ok(pgid)) = (
        fd_str.parse::<libc::c_int>(),
        pgid_str.parse::<libc::pid_t>(),
    ) {
        reaper_main(fd, pgid);
        return;
    }

    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if cli_args
        .first()
        .map(String::as_str)
        .is_some_and(is_client_command)
    {
        if let Err(err) = run_client() {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }

    ur_taking_me_with_you::die_with_parent();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(async {
            if let Err(err) = run_server().await {
                eprintln!("{err}");
                std::process::exit(1);
            }
        });
}

fn is_client_command(value: &str) -> bool {
    matches!(value, "cut" | "sql" | "query" | "snapshot")
}

#[cfg(unix)]
fn reaper_main(pipe_fd: libc::c_int, pgid: libc::pid_t) {
    // Block until the parent closes the write end of the pipe (i.e. parent died).
    let mut buf = [0u8; 1];
    loop {
        let n = unsafe { libc::read(pipe_fd, buf.as_mut_ptr() as *mut _, 1) };
        if n <= 0 {
            break; // EOF or error — parent is gone
        }
    }
    // Kill the entire process group.
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

async fn run_server() -> Result<(), String> {
    let cli = parse_server_cli()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // r[impl config.web.tcp-listen]
    let tcp_addr = std::env::var("MOIRE_LISTEN").unwrap_or_else(|_| "127.0.0.1:9119".into());
    // r[impl config.web.http-listen]
    let http_addr = std::env::var("MOIRE_HTTP").unwrap_or_else(|_| "127.0.0.1:9130".into());
    // r[impl config.web.mcp-listen]
    let mcp_addr = std::env::var("MOIRE_MCP").unwrap_or_else(|_| "127.0.0.1:9131".into());
    // r[impl config.web.vite-addr]
    let vite_addr = std::env::var("MOIRE_VITE_ADDR").unwrap_or_else(|_| DEFAULT_VITE_ADDR.into());
    // r[impl config.web.db-path]
    let db_path =
        PathBuf::from(std::env::var("MOIRE_DB").unwrap_or_else(|_| "moire-web.sqlite".into()));
    let db = Db::new(db_path);
    init_sqlite(&db).map_err(|e| format!("failed to init sqlite at {:?}: {e}", db.path()))?;
    let next_conn_id = load_next_connection_id(&db)
        .map_err(|e| format!("failed to load next connection id at {:?}: {e}", db.path()))?;

    let mut dev_vite_child = None;
    let mut frontend_dist = None;
    let dev_proxy = if cli.dev {
        let child = start_vite_dev_server(&vite_addr).await?;
        info!(vite_addr = %vite_addr, "moire-web --dev launched Vite");
        dev_vite_child = Some(child);
        Some(DevProxyState {
            base_url: Arc::new(format!("http://{vite_addr}")),
        })
    } else {
        let dist = resolve_frontend_dist()?;
        info!(frontend_dist = %dist.display(), "moire-web bundled frontend ready");
        frontend_dist = Some(dist);
        None
    };

    let state = AppState::new(db, next_conn_id, dev_proxy, frontend_dist.clone());

    let tcp_listener = TcpListener::bind(&tcp_addr)
        .await
        .map_err(|e| format!("failed to bind TCP on {tcp_addr}: {e}"))?;
    info!(%tcp_addr, %next_conn_id, "moire-web TCP ingest listener ready");

    let http_listener = TcpListener::bind(&http_addr)
        .await
        .map_err(|e| format!("failed to bind HTTP on {http_addr}: {e}"))?;
    if cli.dev {
        info!(%http_addr, vite_addr = %vite_addr, "moire-web HTTP API + Vite proxy ready");
    } else {
        let dist = frontend_dist
            .as_ref()
            .ok_or("frontend dist was not resolved in non-dev mode")?;
        info!(
            %http_addr,
            frontend_dist = %dist.display(),
            "moire-web HTTP API + bundled frontend ready"
        );
    }
    let mcp_listener = TcpListener::bind(&mcp_addr)
        .await
        .map_err(|e| format!("failed to bind MCP on {mcp_addr}: {e}"))?;
    info!(%mcp_addr, "moire-web MCP listener ready");
    print_startup_hints(
        &http_addr,
        &tcp_addr,
        &mcp_addr,
        if cli.dev { Some(&vite_addr) } else { None },
        frontend_dist.as_deref(),
    );

    let app = build_router(state.clone());

    let _dev_vite_child = dev_vite_child;
    tokio::select! {
        _ = run_tcp_acceptor(tcp_listener, state.clone()) => {}
        result = axum::serve(http_listener, app) => {
            if let Err(e) = result {
                error!(%e, "HTTP server error");
            }
        }
        result = run_mcp_server(mcp_listener, state.clone()) => {
            if let Err(e) = result {
                error!(%e, "MCP server error");
            }
        }
    }
    Ok(())
}

fn parse_server_cli() -> Result<ServerCli, String> {
    let figue_config = args::builder::<ServerCli>()
        .map_err(|e| format!("failed to build CLI schema: {e}"))?
        .cli(|cli| cli.strict())
        .help(|h| {
            h.program_name("moire")
                .description("SQLite-backed moire ingest + API server")
                .version(option_env!("CARGO_PKG_VERSION").unwrap_or("dev"))
        })
        .build();
    let cli = args::Driver::new(figue_config)
        .run()
        .into_result()
        .map_err(|e| e.to_string())?;
    Ok(cli.value)
}

fn run_client() -> Result<(), String> {
    let cli = parse_client_cli()?;
    match cli.command {
        ClientCommand::Cut {
            url,
            poll_ms,
            timeout_ms,
        } => run_cut(url, poll_ms, timeout_ms),
        ClientCommand::Sql { url, query } => run_sql(url, query),
        ClientCommand::Query { url, name, limit } => run_query_pack(url, name, limit),
        ClientCommand::Snapshot { url } => run_snapshot(url),
    }
}

fn parse_client_cli() -> Result<ClientCli, String> {
    let figue_config = args::builder::<ClientCli>()
        .map_err(|e| format!("failed to build CLI schema: {e}"))?
        .cli(|cli| cli.strict())
        .help(|h| {
            h.program_name("moire")
                .description("CLI for moire-web cuts and graph queries")
                .version(option_env!("CARGO_PKG_VERSION").unwrap_or("dev"))
        })
        .build();
    let cli = args::Driver::new(figue_config)
        .run()
        .into_result()
        .map_err(|e| e.to_string())?;
    Ok(cli.value)
}

fn run_cut(
    url: Option<String>,
    poll_ms: Option<u64>,
    timeout_ms: Option<u64>,
) -> Result<(), String> {
    let base_url = url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let poll_ms = poll_ms.unwrap_or(DEFAULT_POLL_MS);
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);

    let trigger_url = format!("{}/api/cuts", base_url.trim_end_matches('/'));
    let trigger_body = http_post_json(&trigger_url, "{}")?;
    let trigger: TriggerCutResponse = facet_json::from_str(&trigger_body)
        .map_err(|e| format!("decode cut trigger response: {e}"))?;

    let status_url = format!(
        "{}/api/cuts/{}",
        base_url.trim_end_matches('/'),
        trigger.cut_id
    );
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let status_body = http_get_text(&status_url)?;
        let status: CutStatusResponse = facet_json::from_str(&status_body)
            .map_err(|e| format!("decode cut status response: {e}"))?;
        if status.pending_connections == 0 {
            println!(
                "{}",
                facet_json::to_string_pretty(&status)
                    .map_err(|e| format!("encode cut status: {e}"))?
            );
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "cut {} timed out after {}ms (pending_connections={})",
                status.cut_id, timeout_ms, status.pending_connections
            ));
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
    }
}

fn run_sql(url: Option<String>, query: String) -> Result<(), String> {
    let base_url = url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

    let req = SqlRequest { sql: query };
    let body = facet_json::to_string(&req).map_err(|e| format!("encode sql request: {e}"))?;
    let url = format!("{}/api/sql", base_url.trim_end_matches('/'));
    let response = http_post_json(&url, &body)?;
    let pretty = facet_json::to_string_pretty(
        &facet_json::from_str::<facet_value::Value>(&response)
            .map_err(|e| format!("decode sql response as json: {e}"))?,
    )
    .map_err(|e| format!("pretty sql response: {e}"))?;
    println!("{pretty}");
    Ok(())
}

fn run_query_pack(url: Option<String>, name: String, limit: Option<u32>) -> Result<(), String> {
    let base_url = url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let limit = limit.unwrap_or(DEFAULT_QUERY_LIMIT);
    let req = QueryRequest {
        name,
        limit: Some(limit),
    };
    let body = facet_json::to_string(&req).map_err(|e| format!("encode query request: {e}"))?;
    let url = format!("{}/api/query", base_url.trim_end_matches('/'));
    let response = http_post_json(&url, &body)?;
    let pretty = facet_json::to_string_pretty(
        &facet_json::from_str::<facet_value::Value>(&response)
            .map_err(|e| format!("decode query response as json: {e}"))?,
    )
    .map_err(|e| format!("pretty query response: {e}"))?;
    println!("{pretty}");
    Ok(())
}

fn run_snapshot(url: Option<String>) -> Result<(), String> {
    let base_url = url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let base_url = base_url.trim_end_matches('/');
    let current_url = format!("{base_url}/api/snapshot/current");

    let response = match ureq::get(&current_url).call() {
        Ok(response) => response
            .into_string()
            .map_err(|e| format!("read GET response body: {e}"))?,
        Err(ureq::Error::Status(404, _)) => {
            let snapshot_url = format!("{base_url}/api/snapshot");
            http_post_json(&snapshot_url, "{}")?
        }
        Err(e) => return Err(format!("GET {current_url}: {e}")),
    };

    let pretty = facet_json::to_string_pretty(
        &facet_json::from_str::<facet_value::Value>(&response)
            .map_err(|e| format!("decode snapshot response as json: {e}"))?,
    )
    .map_err(|e| format!("pretty snapshot response: {e}"))?;
    println!("{pretty}");
    Ok(())
}

fn http_get_text(url: &str) -> Result<String, String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("GET {url}: {e}"))?;
    response
        .into_string()
        .map_err(|e| format!("read GET response body: {e}"))
}

fn http_post_json(url: &str, body: &str) -> Result<String, String> {
    let response = ureq::post(url)
        .set("content-type", "application/json")
        .send_string(body)
        .map_err(|e| format!("POST {url}: {e}"))?;
    response
        .into_string()
        .map_err(|e| format!("read POST response body: {e}"))
}

fn print_startup_hints(
    http_addr: &str,
    tcp_addr: &str,
    mcp_addr: &str,
    vite_addr: Option<&str>,
    frontend_dist: Option<&Path>,
) {
    let mode = if vite_addr.is_some() {
        "dev proxy"
    } else {
        "bundled ui"
    };
    println!();
    println!();

    if let Some(vite_addr) = vite_addr {
        println!("  Vite dev server (managed): http://{vite_addr}");
        println!();
    }
    if let Some(frontend_dist) = frontend_dist {
        println!("  Frontend bundle: {}", frontend_dist.display());
        println!();
    }

    println!("  moire-web ready ({mode})");
    println!();
    println!("  \x1b[32mOpen in browser: http://{http_addr}\x1b[0m");
    println!("  MCP endpoint: \x1b[32mhttp://{mcp_addr}/mcp\x1b[0m");
    println!();
    println!("  Connect apps with:");
    println!("    \x1b[32mMOIRE_DASHBOARD={tcp_addr}\x1b[0m <your-binary>");
    println!();
    println!();
}

fn resolve_frontend_dist() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var(FRONTEND_DIST_ENV) {
        return validate_frontend_dist(PathBuf::from(path), FRONTEND_DIST_ENV);
    }

    let mut candidates: Vec<(&str, PathBuf)> = Vec::new();
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        candidates.push((
            "installed bundle next to executable",
            exe_dir.join("moire-web.dist"),
        ));
    }
    candidates.push((
        "workspace frontend dist",
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../frontend/dist"),
    ));

    for (source, candidate) in &candidates {
        if candidate.exists() {
            return validate_frontend_dist(candidate.clone(), source);
        }
    }

    let tried = candidates
        .iter()
        .map(|(source, candidate)| format!("  - {source}: {}", candidate.display()))
        .collect::<Vec<_>>()
        .join("\n");

    Err(format!(
        "frontend bundle not found. looked in:\n{tried}\nset {FRONTEND_DIST_ENV} to a valid frontend dist directory or run `cargo xtask install`."
    ))
}

fn validate_frontend_dist(path: PathBuf, source: &str) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!(
            "frontend bundle from {source} is not a directory: {}",
            path.display()
        ));
    }
    let index_html = path.join("index.html");
    if !index_html.is_file() {
        return Err(format!(
            "frontend bundle from {source} is missing {}",
            index_html.display()
        ));
    }
    let assets_dir = path.join("assets");
    if !assets_dir.is_dir() {
        return Err(format!(
            "frontend bundle from {source} is missing assets directory {}",
            assets_dir.display()
        ));
    }
    Ok(path)
}
