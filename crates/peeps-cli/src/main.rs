use compact_str::CompactString;
use facet::Facet;
use figue as args;
use std::time::{Duration, Instant};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:9130";
const DEFAULT_POLL_MS: u64 = 100;
const DEFAULT_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_QUERY_LIMIT: u32 = 50;

#[derive(Facet)]
struct TriggerCutResponse {
    cut_id: CompactString,
    requested_at_ns: i64,
    requested_connections: usize,
}

#[derive(Facet)]
struct CutStatusResponse {
    cut_id: CompactString,
    requested_at_ns: i64,
    pending_connections: usize,
    acked_connections: usize,
    pending_conn_ids: Vec<u64>,
}

#[derive(Facet)]
struct SqlRequest {
    sql: CompactString,
}

#[derive(Facet)]
struct QueryRequest {
    name: CompactString,
    #[facet(skip_unless_truthy)]
    limit: Option<u32>,
}

#[derive(Facet, Debug)]
struct Cli {
    #[facet(flatten)]
    builtins: args::FigueBuiltins,
    #[facet(args::subcommand)]
    command: Command,
}

#[derive(Facet, Debug)]
#[repr(u8)]
enum Command {
    Cut {
        #[facet(args::named, default)]
        url: Option<CompactString>,
        #[facet(args::named, default)]
        poll_ms: Option<u64>,
        #[facet(args::named, default)]
        timeout_ms: Option<u64>,
    },
    Sql {
        #[facet(args::named, default)]
        url: Option<CompactString>,
        #[facet(args::named)]
        query: CompactString,
    },
    Query {
        #[facet(args::named, default)]
        url: Option<CompactString>,
        #[facet(args::named)]
        name: CompactString,
        #[facet(args::named, default)]
        limit: Option<u32>,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let figue_config = args::builder::<Cli>()
        .map_err(|e| format!("failed to build CLI schema: {e}"))?
        .cli(|cli| cli.strict())
        .help(|h| {
            h.program_name("peeps")
                .description("CLI for peeps-web cuts and graph queries")
                .version(option_env!("CARGO_PKG_VERSION").unwrap_or("dev"))
        })
        .build();
    let cli = args::Driver::new(figue_config)
        .run()
        .into_result()
        .map_err(|e| e.to_string())?;

    match cli.value.command {
        Command::Cut {
            url,
            poll_ms,
            timeout_ms,
        } => run_cut(url, poll_ms, timeout_ms),
        Command::Sql { url, query } => run_sql(url, query),
        Command::Query { url, name, limit } => run_query_pack(url, name, limit),
    }
}

fn run_cut(
    url: Option<CompactString>,
    poll_ms: Option<u64>,
    timeout_ms: Option<u64>,
) -> Result<(), String> {
    let base_url = url
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
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

fn run_sql(url: Option<CompactString>, query: CompactString) -> Result<(), String> {
    let base_url = url
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

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

fn run_query_pack(
    url: Option<CompactString>,
    name: CompactString,
    limit: Option<u32>,
) -> Result<(), String> {
    let base_url = url
        .map(|value| value.to_string())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
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
