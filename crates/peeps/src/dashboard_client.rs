//! Push-based dashboard client.
//!
//! When `PEEPS_DASHBOARD=<addr>` is set, connects to the dashboard server
//! and pushes periodic snapshots as length-prefixed JSON frames.

use std::collections::HashMap;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Start the background push loop. Spawns a tracked task that reconnects on failure.
pub fn start_push_loop(process_name: String, addr: String, interval: Duration) {
    peeps_tasks::spawn_tracked("peeps_dashboard_push", async move {
        loop {
            match TcpStream::connect(&addr).await {
                Ok(stream) => {
                    eprintln!("[peeps] connected to dashboard at {addr}");
                    if let Err(e) = push_loop(stream, &process_name, interval).await {
                        eprintln!("[peeps] dashboard connection lost: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("[peeps] failed to connect to dashboard at {addr}: {e}");
                }
            }
            // Wait before reconnecting.
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

/// Inner loop: collect dump, serialize, send frame, sleep.
async fn push_loop(
    mut stream: TcpStream,
    process_name: &str,
    interval: Duration,
) -> std::io::Result<()> {
    loop {
        let dump = crate::collect_dump(process_name, HashMap::new());

        let json = facet_json::to_string(&dump).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("serialization error: {e}"),
            )
        })?;

        let len = json.len() as u32;
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(json.as_bytes()).await?;
        stream.flush().await?;

        tokio::time::sleep(interval).await;
    }
}
