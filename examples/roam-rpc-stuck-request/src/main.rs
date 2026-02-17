use std::io;
use std::time::Duration;

use roam::service;
use roam_session::{
    accept_framed, initiate_framed, HandshakeConfig, MessageTransport, NoDispatcher,
};
use roam_wire::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

struct TcpMessageTransport {
    stream: tokio::net::TcpStream,
    last_decoded: Vec<u8>,
}

impl TcpMessageTransport {
    fn new(stream: tokio::net::TcpStream) -> Self {
        Self {
            stream,
            last_decoded: Vec::new(),
        }
    }

    async fn recv_frame(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
        match self.stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => return Err(err),
        }

        let frame_len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; frame_len];
        self.stream.read_exact(&mut payload).await?;
        Ok(Some(payload))
    }
}

impl MessageTransport for TcpMessageTransport {
    async fn send(&mut self, msg: &Message) -> io::Result<()> {
        let payload = facet_postcard::to_vec(msg)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let frame_len = u32::try_from(payload.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "message too large for u32 frame length",
            )
        })?;

        self.stream.write_all(&frame_len.to_le_bytes()).await?;
        self.stream.write_all(&payload).await?;
        self.stream.flush().await
    }

    async fn recv_timeout(&mut self, timeout: Duration) -> io::Result<Option<Message>> {
        match tokio::time::timeout(timeout, self.recv()).await {
            Ok(result) => result,
            Err(_) => Ok(None),
        }
    }

    async fn recv(&mut self) -> io::Result<Option<Message>> {
        let Some(payload) = self.recv_frame().await? else {
            return Ok(None);
        };
        self.last_decoded = payload.clone();
        let msg = facet_postcard::from_slice(&payload)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        Ok(Some(msg))
    }

    fn last_decoded(&self) -> &[u8] {
        &self.last_decoded
    }
}

async fn run_server(addr: &str) {
    peeps::init("example-roam-rpc-stuck-request.server");

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

    let dispatcher = DemoRpcDispatcher::new(DemoService);
    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-server".to_string());

    let transport = TcpMessageTransport::new(stream);
    let (_handle, _incoming, driver) = accept_framed(transport, config, dispatcher)
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
    peeps::init("example-roam-rpc-stuck-request.client");

    let stream = peeps::net::connect(tokio::net::TcpStream::connect(addr), addr, "tcp")
        .await
        .expect("client failed to connect to server");

    let mut config = HandshakeConfig::default();
    config.name = Some("stuck-client".to_string());

    let transport = TcpMessageTransport::new(stream);
    let (client_handle, _incoming, client_driver) =
        initiate_framed(transport, config, NoDispatcher)
            .await
            .expect("client handshake should succeed");

    peeps::spawn_tracked!("roam.client_driver", async move {
        let _ = client_driver.run().await;
    });

    let client = DemoRpcClient::new(client_handle);
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
