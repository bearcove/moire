pub use tokio::sync::oneshot::{error, Sender};

pub use tokio::sync::oneshot::Receiver;

pub fn channel<T>(_name: impl Into<String>) -> (Sender<T>, Receiver<T>) {
    tokio::sync::oneshot::channel()
}
