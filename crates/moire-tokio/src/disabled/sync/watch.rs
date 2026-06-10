pub use tokio::sync::watch::{Receiver, Ref, Sender, error};

pub fn channel<T: Clone>(_name: impl Into<String>, initial: T) -> (Sender<T>, Receiver<T>) {
    tokio::sync::watch::channel(initial)
}
