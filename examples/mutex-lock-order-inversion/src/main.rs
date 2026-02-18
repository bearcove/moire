use std::sync::{Arc, Barrier};
use std::time::Duration;

use tokio::sync::oneshot;

fn spawn_lock_order_worker(
    thread_name: &'static str,
    first_name: &'static str,
    first: Arc<peeps::Mutex<()>>,
    second_name: &'static str,
    second: Arc<peeps::Mutex<()>>,
    ready_barrier: Arc<Barrier>,
    completed_tx: oneshot::Sender<()>,
) {
    std::thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let _first_guard = first.lock();
            println!("{thread_name} locked {first_name}; waiting for peer");

            ready_barrier.wait();

            println!(
                "{thread_name} attempting {second_name}; this should deadlock due to lock-order inversion"
            );
            let _second_guard = second.lock();

            println!(
                "{thread_name} unexpectedly acquired {second_name}; deadlock did not occur"
            );
            let _ = completed_tx.send(());
        })
        .expect("failed to spawn deadlock worker thread");
}

#[tokio::main]
async fn main() {
    peeps::init("example-mutex-lock-order-inversion");

    let left = Arc::new(peeps::Mutex::new("demo.shared.left", ()));
    let right = Arc::new(peeps::Mutex::new("demo.shared.right", ()));
    let ready_barrier = Arc::new(Barrier::new(2));

    let (alpha_done_tx, alpha_done_rx) = oneshot::channel::<()>();
    let (beta_done_tx, beta_done_rx) = oneshot::channel::<()>();

    spawn_lock_order_worker(
        "deadlock.worker.alpha",
        "demo.shared.left",
        Arc::clone(&left),
        "demo.shared.right",
        Arc::clone(&right),
        Arc::clone(&ready_barrier),
        alpha_done_tx,
    );

    spawn_lock_order_worker(
        "deadlock.worker.beta",
        "demo.shared.right",
        Arc::clone(&right),
        "demo.shared.left",
        Arc::clone(&left),
        Arc::clone(&ready_barrier),
        beta_done_tx,
    );

    peeps::spawn_tracked!("observer.alpha_completion", async move {
        let _ = peeps::peep!(alpha_done_rx, "deadlock.alpha.completion.await").await;
        println!("observer.alpha_completion unexpectedly unblocked");
    });

    peeps::spawn_tracked!("observer.beta_completion", async move {
        let _ = peeps::peep!(beta_done_rx, "deadlock.beta.completion.await").await;
        println!("observer.beta_completion unexpectedly unblocked");
    });

    peeps::spawn_tracked!("observer.async_heartbeat", async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            println!("async heartbeat: runtime is alive while worker threads are deadlocked");
        }
    });

    println!(
        "example running. two worker threads should deadlock on demo.shared.left/demo.shared.right"
    );
    println!(
        "inspect deadlock.alpha.completion.await and deadlock.beta.completion.await in peeps-web"
    );
    println!("press Ctrl+C to exit");

    let _ = tokio::signal::ctrl_c().await;
}
