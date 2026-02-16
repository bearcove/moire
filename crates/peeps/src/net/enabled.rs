use std::future::IntoFuture;
use std::time::Instant;

use facet::Facet;
use peeps_types::{Node, NodeKind};

// ── Attrs struct ─────────────────────────────────────────

#[derive(Facet)]
struct NetAttrs {
    #[facet(rename = "net.op")]
    net_op: String,
    #[facet(rename = "net.endpoint")]
    net_endpoint: String,
    #[facet(rename = "net.transport")]
    net_transport: String,
    #[facet(skip_unless_truthy)]
    elapsed_ns: Option<u64>,
}

/// Wrap a connect future with network readiness instrumentation.
///
/// Registers a `NetConnect` node visible in the graph while the connect
/// is pending. Automatically attaches endpoint and transport metadata.
pub async fn connect<F: IntoFuture>(future: F, endpoint: &str, transport: &str) -> F::Output {
    net_wait(
        future,
        endpoint,
        transport,
        NodeKind::NetConnect,
        "net_connect",
    )
    .await
}

/// Wrap an accept future with network readiness instrumentation.
///
/// Registers a `NetAccept` node visible in the graph while the accept
/// is pending.
pub async fn accept<F: IntoFuture>(future: F, endpoint: &str, transport: &str) -> F::Output {
    net_wait(
        future,
        endpoint,
        transport,
        NodeKind::NetAccept,
        "net_accept",
    )
    .await
}

/// Wrap a readable readiness wait with network instrumentation.
///
/// Registers a `NetReadable` node visible in the graph while waiting
/// for the socket to become readable.
pub async fn readable<F: IntoFuture>(future: F, endpoint: &str, transport: &str) -> F::Output {
    net_wait(
        future,
        endpoint,
        transport,
        NodeKind::NetReadable,
        "net_readable",
    )
    .await
}

/// Wrap a writable readiness wait with network instrumentation.
///
/// Registers a `NetWritable` node visible in the graph while waiting
/// for the socket to become writable.
pub async fn writable<F: IntoFuture>(future: F, endpoint: &str, transport: &str) -> F::Output {
    net_wait(
        future,
        endpoint,
        transport,
        NodeKind::NetWritable,
        "net_writable",
    )
    .await
}

async fn net_wait<F: IntoFuture>(
    future: F,
    endpoint: &str,
    transport: &str,
    kind: NodeKind,
    kind_str: &str,
) -> F::Output {
    let node_id = peeps_types::new_node_id(kind_str);
    let label = format!("{kind_str}: {endpoint}");

    let attrs_json = build_begin_attrs(kind_str, endpoint, transport);

    crate::registry::register_node(Node {
        id: node_id.clone(),
        kind,
        label: Some(label.clone()),
        attrs_json,
    });

    // Emit needs edge from current stack top to this wait node.
    let nid = node_id.clone();
    crate::stack::with_top(|src| {
        crate::registry::edge(src, &nid);
    });

    let start = Instant::now();
    let result = future.into_future().await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    // Update the node with final timing, then remove it.
    let final_attrs = build_end_attrs(kind_str, endpoint, transport, elapsed_ns);
    crate::registry::register_node(Node {
        id: node_id.clone(),
        kind,
        label: Some(label),
        attrs_json: final_attrs,
    });
    crate::registry::remove_node(&node_id);

    result
}

fn build_begin_attrs(op: &str, endpoint: &str, transport: &str) -> String {
    let attrs = NetAttrs {
        net_op: op.to_owned(),
        net_endpoint: endpoint.to_owned(),
        net_transport: transport.to_owned(),
        elapsed_ns: None,
    };
    facet_json::to_string(&attrs).unwrap()
}

fn build_end_attrs(op: &str, endpoint: &str, transport: &str, elapsed_ns: u64) -> String {
    let attrs = NetAttrs {
        net_op: op.to_owned(),
        net_endpoint: endpoint.to_owned(),
        net_transport: transport.to_owned(),
        elapsed_ns: Some(elapsed_ns),
    };
    facet_json::to_string(&attrs).unwrap()
}
