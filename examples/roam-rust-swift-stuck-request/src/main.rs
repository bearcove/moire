use std::path::{Path, PathBuf};
use std::process::Stdio;

use roam_stream::{HandshakeConfig, NoDispatcher, accept};
use tokio::process::{Child, Command};

fn swift_package_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("swift")
}

fn spawn_swift_peer(peer_addr: &str) -> std::io::Result<Child> {
    let mut cmd = Command::new("swift");
    cmd.arg("run")
        .arg("--package-path")
        .arg(swift_package_path())
        .arg("rust_swift_peer")
        .env("PEER_ADDR", peer_addr)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    cmd.spawn()
}

#[tokio::main]
async fn main() {
    peeps::init("example-roam-rust-swift-stuck-request.rust");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind listener");
    let addr = listener
        .local_addr()
        .expect("failed to get listener local_addr");

    println!("listening for swift peer on {addr}");

    let mut swift_child = spawn_swift_peer(&addr.to_string())
        .expect("failed to spawn swift runtime peer (requires `swift` toolchain)");

    let (stream, peer_addr) = listener
        .accept()
        .await
        .expect("failed to accept swift peer connection");
    println!("swift peer connected from {peer_addr}");

    let mut config = HandshakeConfig::default();
    config.name = Some("rust-host".to_string());

    let (handle, _incoming, driver) = accept(stream, config, NoDispatcher)
        .await
        .expect("roam handshake with swift peer should succeed");

    peeps::spawn_tracked!("roam.rust_host_driver", async move {
        let _ = driver.run().await;
    });

    let request_handle = handle.clone();
    peeps::spawn_tracked!("rust.calls.swift_noop", async move {
        let _ = peeps::peep!(
            request_handle.call_raw(0xfeed_f00d, "swift.noop.stall", Vec::new()),
            "rpc.call.swift.noop.stall"
        )
        .await;
    });

    println!("example running. rust issues one RPC call that swift intentionally never answers.");
    println!("open peeps-web and inspect request/connection wait edges across this process.");
    println!("press Ctrl+C to exit");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("received Ctrl+C, shutting down");
        }
        status = swift_child.wait() => {
            println!("swift peer exited early: {status:?}");
        }
    }

    let _ = swift_child.kill().await;
    let _ = swift_child.wait().await;
}
