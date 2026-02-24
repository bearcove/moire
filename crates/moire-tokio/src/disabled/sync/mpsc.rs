pub use tokio::sync::mpsc::{
    OwnedPermit, Receiver, Sender, UnboundedReceiver, UnboundedSender, error,
};

pub fn channel<T>(_name: impl Into<String>, capacity: usize) -> (Sender<T>, Receiver<T>) {
    tokio::sync::mpsc::channel(capacity)
}

pub fn unbounded_channel<T>(
    _name: impl Into<String>,
) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    tokio::sync::mpsc::unbounded_channel()
}
