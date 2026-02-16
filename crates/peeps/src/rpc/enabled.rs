use facet::Facet;
use facet_json::RawJson;
use peeps_types::{MetaBuilder, Node, NodeKind};

use super::RpcEvent;

#[derive(Facet)]
struct RpcAttrs<'a> {
    #[facet(rename = "rpc.name")]
    rpc_name: &'a str,
    meta: RawJson<'a>,
}

/// Record or update a request entity node.
///
/// The node remains present until explicitly removed by wrapper code via
/// `peeps::registry::remove_node(entity_id)`.
pub fn record_request(event: RpcEvent<'_>) {
    record(event, NodeKind::Request);
}

/// Record or update a response entity node.
///
/// The node remains present until explicitly removed by wrapper code via
/// `peeps::registry::remove_node(entity_id)`.
pub fn record_response(event: RpcEvent<'_>) {
    record(event, NodeKind::Response);
}

/// Record or update a request node using stack-built metadata.
///
/// Builds `attrs_json` as:
/// `{"rpc.name":"...","meta":{...}}`
pub fn record_request_with_meta<const N: usize>(
    entity_id: &str,
    name: &str,
    meta: MetaBuilder<'_, N>,
    parent_entity_id: Option<&str>,
) {
    let attrs_json = attrs_json_with_meta(name, &meta.to_json_object());
    record_request(RpcEvent {
        entity_id,
        name,
        attrs_json: &attrs_json,
        parent_entity_id,
    });
}

/// Record or update a response node using stack-built metadata.
///
/// Builds `attrs_json` as:
/// `{"rpc.name":"...","meta":{...}}`
pub fn record_response_with_meta<const N: usize>(
    entity_id: &str,
    name: &str,
    meta: MetaBuilder<'_, N>,
    parent_entity_id: Option<&str>,
) {
    let attrs_json = attrs_json_with_meta(name, &meta.to_json_object());
    record_response(RpcEvent {
        entity_id,
        name,
        attrs_json: &attrs_json,
        parent_entity_id,
    });
}

fn record(event: RpcEvent<'_>, kind: NodeKind) {
    crate::registry::register_node(Node {
        id: event.entity_id.to_string(),
        kind,
        label: Some(event.name.to_string()),
        attrs_json: event.attrs_json.to_string(),
    });

    let parent = event
        .parent_entity_id
        .map(ToOwned::to_owned)
        .or_else(crate::stack::capture_top);
    if let Some(parent_id) = parent {
        if parent_id != event.entity_id {
            crate::registry::edge(&parent_id, event.entity_id);
            crate::registry::touch_edge(&parent_id, event.entity_id);
        }
    }
}

fn attrs_json_with_meta(name: &str, meta_json: &str) -> String {
    let attrs = RpcAttrs {
        rpc_name: name,
        meta: RawJson::new(meta_json),
    };
    facet_json::to_string(&attrs).unwrap()
}
