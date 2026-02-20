pub use tokio::sync::broadcast::{error, Receiver, Sender};

pub fn channel<T: Clone>(_name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    tokio::sync::broadcast::channel(capacity)
}
