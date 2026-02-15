#[inline]
pub fn snapshot_lock_diagnostics() -> crate::LockSnapshot {
    crate::LockSnapshot { locks: Vec::new() }
}

#[inline]
pub fn dump_lock_diagnostics() -> String {
    String::new()
}

#[inline(always)]
pub fn emit_lock_graph(_process_name: &str, _proc_key: &str) -> peeps_types::GraphSnapshot {
    peeps_types::GraphSnapshot::empty()
}
