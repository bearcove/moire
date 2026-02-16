use peeps_types::{GraphSnapshot, Node};

#[inline(always)]
pub(crate) fn init(_process_name: &str, _proc_key: &str) {}

#[inline(always)]
pub(crate) fn process_name() -> Option<&'static str> {
    None
}

#[inline(always)]
pub(crate) fn proc_key() -> Option<&'static str> {
    None
}

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
pub fn remove_node(_id: &str) {}

#[inline(always)]
pub fn record_event(_entity_id: &str, _name: &str, _attrs_json: impl Into<String>) {}

#[inline(always)]
pub(crate) fn emit_graph() -> GraphSnapshot {
    GraphSnapshot::default()
}
