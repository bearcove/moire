use std::env;
use std::io;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;

const ADDR: &str = "127.0.0.1:9661";

#[tokio::main]
async fn main() -> io::Result<()> {
    if env::args().nth(1).as_deref() == Some("--server") {
        run_server().await
    } else {
        run_client().await
    }
}

async fn run_server() -> io::Result<()> {
    peeps::init("example-rpc-server");
    let listener = TcpListener::bind(ADDR).await?;
    println!("server listening on {ADDR}");

    loop {
        let (stream, peer) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream).await {
                eprintln!("server connection {peer} error: {err}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read_count = peeps::instrument_future_named("rpc.server.read_frame", reader.read_line(&mut line)).await?;
    if read_count == 0 {
        return Ok(());
    }

    let mut fields = line.trim_end().splitn(3, '\t');
    let request_id = fields.next().unwrap_or_default();
    let method = fields.next().unwrap_or("unknown");
    let payload = fields.next().unwrap_or_default();

    let request_ref = peeps::entity_ref_from_wire(request_id.to_string());
    let response = peeps::rpc_response_for(method.to_string(), &request_ref);
    let mut stream = reader.into_inner();

    match method {
        "demo.echo" => {
            let reply = format!("ok\t{payload}\n");
            peeps::instrument_future_on("rpc.server.write_echo", response.handle(), stream.write_all(reply.as_bytes())).await?;
            response.mark_ok();
        }
        "demo.sleepy_forever" => {
            peeps::instrument_future_on(
                "rpc.server.sleepy_forever",
                response.handle(),
                tokio::time::sleep(Duration::from_secs(3600)),
            )
            .await;
            response.mark_cancelled();
            let _ = peeps::instrument_future_on(
                "rpc.server.write_late_cancel",
                response.handle(),
                stream.write_all(b"cancelled\twoke_up\n"),
            )
            .await;
        }
        _ => {
            response.mark_error();
            let _ = peeps::instrument_future_on(
                "rpc.server.write_error",
                response.handle(),
                stream.write_all(b"error\tunknown_method\n"),
            )
            .await;
        }
    }

    Ok(())
}

async fn run_client() -> io::Result<()> {
    peeps::init("example-rpc-client");

    let exe = env::current_exe()?;
    let mut server = Command::new(exe).arg("--server").spawn()?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let echo = do_rpc("demo.echo", "hello from client").await?;
    println!("echo response: {echo}");

    tokio::spawn(async {
        if let Err(err) = do_rpc("demo.sleepy_forever", "stuck request").await {
            eprintln!("blocked rpc error: {err}");
        }
    });

    println!("started one blocked rpc call");
    println!("press Ctrl+C to stop both processes");
    let _ = tokio::signal::ctrl_c().await;

    let _ = server.start_kill();
    let _ = server.wait().await;
    Ok(())
}

async fn do_rpc(method: &str, payload: &str) -> io::Result<String> {
    let request = peeps::rpc_request(method.to_string(), payload.to_string());
    let mut stream = peeps::instrument_future_on(
        "rpc.client.connect",
        request.handle(),
        TcpStream::connect(ADDR),
    )
    .await?;

    let frame = format!("{}\t{method}\t{payload}\n", request.id_for_wire());
    peeps::instrument_future_on(
        "rpc.client.write_frame",
        request.handle(),
        stream.write_all(frame.as_bytes()),
    )
    .await?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    peeps::instrument_future_on(
        "rpc.client.read_frame",
        request.handle(),
        reader.read_line(&mut response),
    )
    .await?;

    Ok(response.trim_end().to_string())
}
