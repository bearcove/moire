pub use super::channels::{
    channel, oneshot_channel, unbounded_channel, watch_channel, OneshotReceiver, OneshotSender,
    Receiver, Sender, UnboundedReceiver, UnboundedSender, WatchReceiver, WatchSender,
};
pub use super::notify::DiagnosticNotify;
pub use super::oncecell::OnceCell;
pub use super::semaphore::DiagnosticSemaphore;
pub use super::timers::{interval, interval_at, DiagnosticInterval};

pub(crate) fn emit_into_graph(graph: &mut peeps_types_legacy::GraphSnapshot) {
    super::channels::emit_channel_nodes(graph);
    super::semaphore::emit_semaphore_nodes(graph);
    super::oncecell::emit_oncecell_nodes(graph);
    super::notify::emit_notify_nodes(graph);
    super::timers::emit_interval_nodes(graph);
}
