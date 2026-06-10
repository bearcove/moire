pub use tokio::sync::broadcast::{Receiver, Sender, error};

pub fn channel<T: Clone>(_name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    tokio::sync::broadcast::channel(capacity)
}
