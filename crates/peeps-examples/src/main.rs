use compact_str::CompactString;
use facet::Facet;
use figue as args;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::Duration;

mod scenarios;

type AnyResult<T> = Result<T, String>;

#[derive(Facet, Debug)]
struct Cli {
    #[facet(flatten)]
    builtins: args::FigueBuiltins,
    #[facet(args::named, default)]
    no_web: bool,
    #[facet(args::named, default)]
    no_open: bool,
    #[facet(args::named, default)]
    peeps_listen: Option<CompactString>,
    #[facet(args::named, default)]
    peeps_http: Option<CompactString>,
    #[facet(args::subcommand)]
    command: CommandKind,
}

#[derive(Facet, Debug)]
#[repr(u8)]
enum CommandKind {
    ChannelFullStall,
    MutexLockOrderInversion,
    OneshotSenderLostInMap,
    RoamRpcStuckRequest,
    RoamRpcStuckRequestClient {
        #[facet(args::named)]
        peer_addr: CompactString,
    },
    RoamRustSwiftStuckRequest,
    SemaphoreStarvation,
}

struct Config {
    root_dir: PathBuf,
    peeps_listen: String,
    peeps_http: String,
    no_open: bool,
    no_web: bool,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> AnyResult<()> {
    let cli = parse_cli()?;

    let cfg = config_from_cli(&cli);

    let mut backend = if cfg.no_web {
        None
    } else {
        ensure_backend_not_running(&cfg.peeps_http)?;
        println!(
            "Starting peeps-web on {} (ingest: {})",
            cfg.peeps_http, cfg.peeps_listen
        );
        let mut child = spawn_backend(&cfg)?;
        wait_for_backend_health(&cfg.peeps_http, &mut child)?;
        if !cfg.no_open {
            open_browser(&format!("http://{}", cfg.peeps_http));
        }
        std::env::set_var("PEEPS_DASHBOARD", &cfg.peeps_listen);
        Some(child)
    };

    let run_result = dispatch_command(&cfg.root_dir, cli.command).await;

    if let Some(child) = backend.as_mut() {
        terminate_child_group(child);
    }

    run_result
}

fn parse_cli() -> AnyResult<Cli> {
    let figue_config = args::builder::<Cli>()
        .map_err(|e| format!("failed to build CLI schema: {e}"))?
        .cli(|cli| cli.strict())
        .help(|h| {
            h.program_name("peeps-examples")
                .description("Run peeps scenarios as subcommands")
                .version(option_env!("CARGO_PKG_VERSION").unwrap_or("dev"))
        })
        .build();

    args::Driver::new(figue_config)
        .run()
        .into_result()
        .map(|v| v.value)
        .map_err(|e| e.to_string())
}

fn config_from_cli(cli: &Cli) -> Config {
    let peeps_listen = cli
        .peeps_listen
        .as_ref()
        .map(|v| v.to_string())
        .or_else(|| std::env::var("PEEPS_LISTEN").ok())
        .unwrap_or_else(|| "127.0.0.1:9119".to_owned());

    let peeps_http = cli
        .peeps_http
        .as_ref()
        .map(|v| v.to_string())
        .or_else(|| std::env::var("PEEPS_HTTP").ok())
        .unwrap_or_else(|| "127.0.0.1:9130".to_owned());

    Config {
        root_dir: workspace_root(),
        peeps_listen,
        peeps_http,
        no_open: cli.no_open,
        no_web: cli.no_web,
    }
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crate must live under <root>/crates/<name>")
        .to_path_buf()
}

async fn dispatch_command(root_dir: &std::path::Path, command: CommandKind) -> AnyResult<()> {
    match command {
        CommandKind::ChannelFullStall => scenarios::channel_full_stall::run().await,
        CommandKind::MutexLockOrderInversion => scenarios::mutex_lock_order_inversion::run().await,
        CommandKind::OneshotSenderLostInMap => scenarios::oneshot_sender_lost_in_map::run().await,
        CommandKind::RoamRpcStuckRequest => scenarios::roam_rpc_stuck_request::run().await,
        CommandKind::RoamRpcStuckRequestClient { peer_addr } => {
            scenarios::roam_rpc_stuck_request::run_client_process(peer_addr.to_string()).await
        }
        CommandKind::RoamRustSwiftStuckRequest => {
            scenarios::roam_rust_swift_stuck_request::run(root_dir).await
        }
        CommandKind::SemaphoreStarvation => scenarios::semaphore_starvation::run().await,
    }
}

fn ensure_backend_not_running(peeps_http: &str) -> AnyResult<()> {
    let health_url = format!("http://{peeps_http}/health");
    if http_get_ok(&health_url) {
        return Err(format!(
            "A peeps-web backend is already running at http://{peeps_http}. Stop it first, or set PEEPS_HTTP/PEEPS_LISTEN to alternate ports."
        ));
    }
    Ok(())
}

fn wait_for_backend_health(peeps_http: &str, backend: &mut Child) -> AnyResult<()> {
    let health_url = format!("http://{peeps_http}/health");
    for _ in 0..100 {
        if http_get_ok(&health_url) {
            return Ok(());
        }
        if let Some(status) = backend.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "peeps-web backend exited before becoming healthy: {}",
                format_status(status)
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "Timed out waiting for peeps-web backend health at {health_url}"
    ))
}

fn http_get_ok(url: &str) -> bool {
    ureq::get(url)
        .call()
        .map(|r| r.status() < 400)
        .unwrap_or(false)
}

fn spawn_backend(cfg: &Config) -> AnyResult<Child> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&cfg.root_dir)
        .args(["run", "-p", "peeps-web", "--", "--dev"])
        .env("PEEPS_LISTEN", &cfg.peeps_listen)
        .env("PEEPS_HTTP", &cfg.peeps_http)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    configure_process_group(&mut cmd);
    cmd.spawn()
        .map_err(|e| format!("failed to spawn peeps-web: {e}"))
}

#[cfg(unix)]
fn configure_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_cmd: &mut Command) {}

#[cfg(unix)]
fn terminate_child_group(child: &mut Child) {
    let pid = child.id() as i32;

    if child.try_wait().ok().flatten().is_none() {
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }

        for _ in 0..10 {
            if child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }

    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate_child_group(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn format_status(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit code {code}"),
        None => "signal".to_owned(),
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}
