use std::time::Duration;

#[tokio::main]
async fn main() {
    peeps::init("example-channel-full-stall");

    let (tx, mut rx) = peeps::channel!("demo.work_queue", 16);

    peeps::spawn_tracked!("stalled_receiver", async move {
        println!("receiver started but is intentionally not draining the queue");
        tokio::time::sleep(Duration::from_secs(3600)).await;
        let _ = rx.recv().await;
    });

    peeps::spawn_tracked!("bounded_sender", async move {
        for i in 0_u32..16 {
            tx.send(i)
                .await
                .expect("channel is open while pre-filling buffer");
            println!("sent prefill item {i}");
        }

        println!("attempting 17th send; this should block because capacity is 16 and receiver is stalled");

        tx.send(16).await.expect("send unexpectedly unblocked");
    });

    println!("example running. open peeps-web and inspect demo.work_queue");
    println!("press Ctrl+C to exit");
    let _ = tokio::signal::ctrl_c().await;
}
