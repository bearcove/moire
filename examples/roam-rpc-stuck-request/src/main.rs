use std::io;
use std::time::Duration;

use roam::service;
use roam_stream::{Connector, HandshakeConfig, NoDispatcher, accept, connect};
use tokio::net::TcpStream;

#[service]
trait DemoRpc {
    async fn sleepy_forever(&self) -> String;
}

#[derive(Clone, Default)]
struct DemoService;

impl DemoRpc for DemoService {
    async fn sleepy_forever(&self, _cx: &roam::Context) -> String {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}

struct TcpConnector {
    addr: String,
}

impl Connector for TcpConnector {
    type Transport = TcpStream;

    async fn connect(&self) -> io::Result<TcpStream> {
        TcpStream::connect(&self.addr).await
    }
}

async fn run_server(addr: &str) {
    peeps::init!();

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("server failed to bind tcp listener");

    let bound_addr = listener.local_addr().expect("failed to get local addr");
    println!("server listening on {bound_addr}");

    let exe = std::env::current_exe().expect("failed to get current exe path");
    let mut child = tokio::process::Command::new(&exe)
        .arg(bound_addr.to_string())
        .spawn()
        .expect("failed to spawn client process");

    let (stream, peer_addr) = listener
        .accept()
        .await
        .expect("server failed to accept client connection");

    println!("client connected from {peer_addr}");

    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-server".to_string());

    let dispatcher = DemoRpcDispatcher::new(DemoService);
    let (_handle, _incoming, driver) = accept(stream, config, dispatcher)
        .await
        .expect("server handshake should succeed");

    peeps::spawn_tracked!("roam.server_driver", async move {
        let _ = driver.run().await;
    });

    println!("server ready: requests to sleepy_forever will stall forever");
    println!("press Ctrl+C to exit");
    let _ = tokio::signal::ctrl_c().await;
    child.kill().await.ok();
}

async fn run_client(addr: &str) {
    peeps::init!();

    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-client".to_string());

    let connector = TcpConnector {
        addr: addr.to_string(),
    };
    let client_transport = connect(connector, config, NoDispatcher);

    let client = DemoRpcClient::new(client_transport);
    peeps::spawn_tracked!("roam.client.request_task", async move {
        client
            .sleepy_forever()
            .await
            .expect("request unexpectedly completed");
    });

    println!("client: sent one sleepy_forever RPC request (intentionally stuck)");
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() {
    match std::env::args().nth(1) {
        None => run_server("127.0.0.1:0").await,
        Some(addr) => run_client(&addr).await,
    }
}
