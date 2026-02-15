use peeps_types::GraphSnapshot;

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
pub(crate) fn edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub(crate) fn remove_edge(_src: &str, _dst: &str) {}

#[inline(always)]
pub(crate) fn remove_edges_from(_src: &str) {}

#[inline(always)]
pub(crate) fn emit_graph() -> GraphSnapshot {
    GraphSnapshot::default()
}
