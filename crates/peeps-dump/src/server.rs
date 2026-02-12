use std::collections::HashMap;
use std::sync::Arc;

use peeps_types::ProcessDump;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};

/// Key for identifying a process: (process_name, pid).
type ProcessKey = (String, u32);

/// Shared dashboard state, holding the latest dump from each connected process.
pub struct DashboardState {
    dumps: Mutex<HashMap<ProcessKey, ProcessDump>>,
    notify: broadcast::Sender<()>,
}

impl DashboardState {
    pub fn new() -> Self {
        let (notify, _) = broadcast::channel(16);
        Self {
            dumps: Mutex::new(HashMap::new()),
            notify,
        }
    }

    /// Insert or update a dump. Notifies subscribers.
    pub async fn upsert_dump(&self, dump: ProcessDump) {
        let key = (dump.process_name.clone(), dump.pid);
        self.dumps.lock().await.insert(key, dump);
        let _ = self.notify.send(());
    }

    /// Get all current dumps as a sorted vec.
    pub async fn all_dumps(&self) -> Vec<ProcessDump> {
        let map = self.dumps.lock().await;
        let mut dumps: Vec<ProcessDump> = map.values().cloned().collect();
        dumps.sort_by(|a, b| a.process_name.cmp(&b.process_name));
        dumps
    }

    /// Subscribe to change notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.notify.subscribe()
    }
}

/// Accept TCP connections and spawn a reader task for each.
pub async fn run_tcp_acceptor(listener: TcpListener, state: Arc<DashboardState>) {
    let max_frame_bytes = max_frame_bytes_from_env();
    eprintln!("[peeps] max frame size set to {max_frame_bytes} bytes");
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                eprintln!("[peeps] TCP connection from {addr}");
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = handle_tcp_connection(stream, &state, max_frame_bytes).await {
                        eprintln!("[peeps] connection from {addr} closed: {e}");
                    } else {
                        eprintln!("[peeps] connection from {addr} closed");
                    }
                });
            }
            Err(e) => {
                eprintln!("[peeps] TCP accept error: {e}");
            }
        }
    }
}

/// Read length-prefixed JSON frames from a single TCP connection.
///
/// Wire format: `[u32 big-endian length][UTF-8 JSON ProcessDump]`
async fn handle_tcp_connection(
    mut stream: TcpStream,
    state: &DashboardState,
    max_frame_bytes: usize,
) -> std::io::Result<()> {
    loop {
        // Read 4-byte length prefix (big-endian u32).
        let len = stream.read_u32().await?;

        if len == 0 {
            continue;
        }

        // Sanity limit to avoid unbounded memory growth on malformed clients.
        if (len as usize) > max_frame_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("frame too large: {len} bytes (max {max_frame_bytes})"),
            ));
        }

        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let json = match std::str::from_utf8(&buf) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[peeps] invalid UTF-8 in frame: {e}");
                continue;
            }
        };

        match facet_json::from_str::<ProcessDump>(json) {
            Ok(dump) => {
                eprintln!(
                    "[peeps] dump from {} (pid {}): {} tasks, {} threads",
                    dump.process_name,
                    dump.pid,
                    dump.tasks.len(),
                    dump.threads.len()
                );
                state.upsert_dump(dump).await;
            }
            Err(e) => {
                eprintln!("[peeps] failed to parse dump frame: {e}");
            }
        }
    }
}

fn max_frame_bytes_from_env() -> usize {
    const DEFAULT_MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
    match std::env::var("PEEPS_MAX_FRAME_BYTES") {
        Ok(raw) => match raw.parse::<usize>() {
            Ok(v) if v > 0 => v,
            _ => {
                eprintln!(
                    "[peeps] invalid PEEPS_MAX_FRAME_BYTES={raw:?}, using default {}",
                    DEFAULT_MAX_FRAME_BYTES
                );
                DEFAULT_MAX_FRAME_BYTES
            }
        },
        Err(_) => DEFAULT_MAX_FRAME_BYTES,
    }
}
