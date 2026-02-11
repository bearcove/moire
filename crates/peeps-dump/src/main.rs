//! peeps CLI tool
//!
//! Commands:
//! - `peeps` - Collect and serve dashboard (like `vx debug`)
//! - `peeps clean` - Clean stale dumps

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "clean" {
        peeps::clean_dumps();
        eprintln!("[peeps] Cleaned dump directory");
        return;
    }

    // Default: serve dashboard
    eprintln!("[peeps] Reading dumps from {}", peeps::DUMP_DIR);
    let dumps = peeps::read_all_dumps();

    if dumps.is_empty() {
        eprintln!("[peeps] No dumps found. Trigger with: kill -SIGUSR1 <pid>");
        std::process::exit(1);
    }

    eprintln!("[peeps] Found {} dumps:", dumps.len());
    for dump in &dumps {
        eprintln!(
            "  {} (pid {}): {} tasks, {} threads",
            dump.process_name,
            dump.pid,
            dump.tasks.len(),
            dump.threads.len()
        );
    }

    peeps::serve_dashboard(dumps).await.unwrap();
}
