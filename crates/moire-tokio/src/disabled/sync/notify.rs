use std::fmt;
use std::sync::Arc;

/// Pass-through `tokio::sync::Notify` wrapper, accepting a name parameter for API parity.
#[derive(Clone)]
pub struct Notify(Arc<tokio::sync::Notify>);

impl Notify {
    pub fn new(_name: impl Into<String>) -> Self {
        Self(Arc::new(tokio::sync::Notify::new()))
    }

    pub async fn notified(&self) {
        self.0.notified().await
    }

    pub fn notify_one(&self) {
        self.0.notify_one()
    }

    pub fn notify_waiters(&self) {
        self.0.notify_waiters()
    }
}

impl fmt::Debug for Notify {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
