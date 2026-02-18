use std::io;
use std::time::Duration;

use roam::service;
use roam_stream::{accept, connect, Connector, HandshakeConfig, NoDispatcher};
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

pub async fn run() -> Result<(), String> {
    peeps::init!();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("server failed to bind tcp listener: {e}"))?;
    let bound_addr = listener
        .local_addr()
        .map_err(|e| format!("failed to get local addr: {e}"))?;
    println!("server listening on {bound_addr}");

    let client_addr = bound_addr.to_string();
    peeps::spawn_tracked!("roam.client.bootstrap", async move {
        let _ = run_client(client_addr).await;
    });

    let (stream, peer_addr) = listener
        .accept()
        .await
        .map_err(|e| format!("server failed to accept client connection: {e}"))?;

    println!("client connected from {peer_addr}");

    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-server".to_string());

    let dispatcher = DemoRpcDispatcher::new(DemoService);
    let (_handle, _incoming, driver) = accept(stream, config, dispatcher)
        .await
        .map_err(|e| format!("server handshake should succeed: {e}"))?;

    peeps::spawn_tracked!("roam.server_driver", async move {
        let _ = driver.run().await;
    });

    println!("server ready: requests to sleepy_forever will stall forever");
    println!("press Ctrl+C to exit");
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| format!("failed waiting for Ctrl+C: {e}"))?;
    Ok(())
}

async fn run_client(addr: String) -> Result<(), String> {
    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-client".to_string());

    let connector = TcpConnector { addr };
    let client_transport = connect(connector, config, NoDispatcher);

    let client = DemoRpcClient::new(client_transport);
    peeps::spawn_tracked!("roam.client.request_task", async move {
        client
            .sleepy_forever()
            .await
            .expect("request unexpectedly completed");
    });

    println!("client: sent one sleepy_forever RPC request (intentionally stuck)");
    Ok(())
}
