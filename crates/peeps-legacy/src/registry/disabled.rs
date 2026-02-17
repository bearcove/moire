use peeps_types_legacy::{GraphSnapshot, Node, NodeKind};

#[inline(always)]
pub(crate) fn init(_process_name: &str, _proc_key: &str) {}

#[inline(always)]
pub fn edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub fn remove_edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub fn remove_edges_from(_src: &str) {}

#[inline(always)]
pub fn remove_edges_to(_dst: &str) {}

#[inline(always)]
pub fn touch_edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub fn remove_touch_edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub fn remove_touch_edges_from(_src: &str) {}

#[inline(always)]
pub fn remove_touch_edges_to(_dst: &str) {}

#[inline(always)]
pub fn spawn_edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub fn remove_spawn_edges_to(_dst: &str) {}

#[inline(always)]
pub fn register_node(_node: Node) {}

#[inline(always)]
pub fn make_node(
    id: impl Into<String>,
    kind: NodeKind,
    label: Option<String>,
    attrs_json: impl Into<String>,
    _created_at: i64,
) -> Node {
    Node {
        id: id.into(),
        kind,
        label,
        attrs_json: attrs_json.into(),
    }
}

#[inline(always)]
pub fn created_at_now_ns() -> i64 {
    0
}

#[inline(always)]
pub fn remove_node(_id: &str) {}

#[inline(always)]
pub fn record_event(_entity_id: &str, _name: &str, _attrs_json: impl Into<String>) {}

#[inline(always)]
pub(crate) fn emit_graph() -> GraphSnapshot {
    GraphSnapshot::default()
}
