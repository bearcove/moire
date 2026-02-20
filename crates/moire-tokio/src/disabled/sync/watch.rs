pub use tokio::sync::watch::{error, Ref, Receiver, Sender};

pub fn channel<T: Clone>(_name: impl Into<String>, initial: T) -> (Sender<T>, Receiver<T>) {
    tokio::sync::watch::channel(initial)
}
