use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tokio::process::Child;
use tokio::time::sleep;

use crate::app::AppState;

pub mod vite;

const PROXY_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_VITE_ADDR: &str = "[::]:9131";
const REAPER_PIPE_FD_ENV: &str = "MOIRE_REAPER_PIPE_FD";
const REAPER_PGID_ENV: &str = "MOIRE_REAPER_PGID";

pub async fn proxy_vite(
    State(state): State<AppState>,
    request: Request,
) -> axum::response::Response {
    let Some(proxy) = state.dev_proxy.clone() else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    vite::proxy_vite_request(proxy.base_url.as_str(), request, PROXY_BODY_LIMIT_BYTES).await
}

pub async fn start_vite_dev_server(vite_addr: &str) -> Result<Child, String> {
    let socket_addr = std::net::SocketAddr::from_str(vite_addr)
        .map_err(|e| format!("invalid MOIRE_VITE_ADDR '{vite_addr}': {e}"))?;
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frontend_dir = workspace_root.join("frontend");
    if !frontend_dir.is_dir() {
        return Err(format!(
            "frontend directory not found at {}",
            frontend_dir.display()
        ));
    }

    ensure_frontend_deps(&workspace_root).await?;

    let mut command = tokio::process::Command::new("pnpm");
    command
        .arg("--filter")
        .arg("moire-frontend")
        .arg("dev")
        .arg("--")
        .arg("--host")
        .arg(socket_addr.ip().to_string())
        .arg("--port")
        .arg(socket_addr.port().to_string())
        .arg("--strictPort")
        .current_dir(&workspace_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    #[cfg(unix)]
    command.process_group(0);

    let child = command.spawn().map_err(|e| {
        format!(
            "failed to launch Vite via pnpm in {}: {e}",
            workspace_root.display()
        )
    })?;

    #[cfg(unix)]
    {
        let vite_pgid = child.id().ok_or("Vite child has no PID")? as libc::pid_t;
        spawn_vite_reaper(vite_pgid)?;
    }

    wait_for_tcp_ready(vite_addr, Duration::from_secs(20)).await?;
    Ok(child)
}

#[cfg(unix)]
fn spawn_vite_reaper(vite_pgid: libc::pid_t) -> Result<(), String> {
    use std::os::fd::FromRawFd;

    let mut fds = [0 as libc::c_int; 2];
    let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
    if ret != 0 {
        return Err(format!(
            "failed to create reaper pipe: {}",
            std::io::Error::last_os_error()
        ));
    }
    let read_fd = fds[0];
    let write_fd = fds[1];

    unsafe {
        let flags = libc::fcntl(read_fd, libc::F_GETFD);
        libc::fcntl(read_fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
    }
    unsafe {
        let flags = libc::fcntl(write_fd, libc::F_GETFD);
        libc::fcntl(write_fd, libc::F_SETFD, flags | libc::FD_CLOEXEC);
    }

    let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;
    std::process::Command::new(exe)
        .env(REAPER_PIPE_FD_ENV, read_fd.to_string())
        .env(REAPER_PGID_ENV, vite_pgid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn vite reaper: {e}"))?;

    unsafe { libc::close(read_fd) };
    std::mem::forget(unsafe { std::fs::File::from_raw_fd(write_fd) });

    Ok(())
}

async fn ensure_frontend_deps(workspace_root: &PathBuf) -> Result<(), String> {
    let vite_ready = tokio::process::Command::new("pnpm")
        .arg("--filter")
        .arg("moire-frontend")
        .arg("exec")
        .arg("vite")
        .arg("--version")
        .current_dir(workspace_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if vite_ready {
        return Ok(());
    }

    tracing::info!(
        workspace = %workspace_root.display(),
        "frontend dependencies missing, running pnpm install"
    );

    let status = tokio::process::Command::new("pnpm")
        .arg("install")
        .current_dir(workspace_root)
        .env("CI", "true")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .map_err(|e| {
            format!(
                "failed to run pnpm install in {}: {e}",
                workspace_root.display()
            )
        })?;

    if !status.success() {
        return Err(format!(
            "pnpm install failed in {} (status: {status})",
            workspace_root.display()
        ));
    }

    let vite_ready = tokio::process::Command::new("pnpm")
        .arg("--filter")
        .arg("moire-frontend")
        .arg("exec")
        .arg("vite")
        .arg("--version")
        .current_dir(workspace_root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if !vite_ready {
        return Err(
            "pnpm install succeeded but vite is still unavailable for moire-frontend".to_string(),
        );
    }

    Ok(())
}

async fn wait_for_tcp_ready(addr: &str, timeout: Duration) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(stream) => {
                drop(stream);
                return Ok(());
            }
            Err(err) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(format!("timed out waiting for Vite at {addr}: {err}"));
                }
            }
        }
        sleep(Duration::from_millis(150)).await;
    }
}
