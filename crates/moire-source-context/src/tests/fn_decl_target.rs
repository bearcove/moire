fn helper() {}

👉
fn spawn_worker(
    task_name: &'static str,
    mutex: Arc<SyncMutex<()>>,
) {
    moire::task::spawn(async move {
        let _guard = mutex.lock();
    }.named(task_name));
}

fn other() {}
