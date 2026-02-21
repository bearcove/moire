use std::collections::BTreeMap;

use facet::Facet;
use moire_trace_types::{BacktraceId, RelPc};
use moire_types::ProcessId;
use rusqlite_facet::StatementFacetExt;

use crate::db::Db;

#[derive(Facet, Clone)]
pub(crate) struct StoredBacktraceFrameRow {
    pub(crate) process_id: ProcessId,
    pub(crate) frame_index: u32,
    pub(crate) module_path: String,
    pub(crate) module_identity: String,
    pub(crate) rel_pc: RelPc,
}

#[derive(Facet, Clone)]
pub(crate) struct SymbolicatedFrameRow {
    pub(crate) process_id: ProcessId,
    pub(crate) frame_index: u32,
    pub(crate) module_path: String,
    pub(crate) rel_pc: RelPc,
    pub(crate) status: String,
    pub(crate) function_name: Option<String>,
    pub(crate) source_file_path: Option<String>,
    pub(crate) source_line: Option<i64>,
    pub(crate) unresolved_reason: Option<String>,
}

pub(crate) struct BacktraceFrameBatch {
    pub(crate) backtrace_id: BacktraceId,
    pub(crate) raw_rows: Vec<StoredBacktraceFrameRow>,
    pub(crate) symbolicated_by_index: BTreeMap<u32, SymbolicatedFrameRow>,
}

#[derive(Facet)]
struct BacktraceFrameParams {
    backtrace_id: BacktraceId,
}

pub(crate) fn load_backtrace_frame_batches(
    db: &Db,
    backtrace_ids: &[BacktraceId],
) -> Result<Vec<BacktraceFrameBatch>, String> {
    let conn = db.open()?;

    let mut raw_stmt = conn
        .prepare(
            "SELECT process_id, frame_index, module_path, module_identity, rel_pc
             FROM backtrace_frames
             WHERE backtrace_id = :backtrace_id
             ORDER BY frame_index ASC",
        )
        .map_err(|error| format!("prepare backtrace_frames read: {error}"))?;
    let mut symbol_stmt = conn
        .prepare(
            "SELECT process_id, frame_index, module_path, rel_pc, status, function_name, source_file_path, source_line, unresolved_reason
             FROM symbolicated_frames
             WHERE backtrace_id = :backtrace_id",
        )
        .map_err(|error| format!("prepare symbolicated_frames read: {error}"))?;

    let mut batches = Vec::with_capacity(backtrace_ids.len());
    for backtrace_id in backtrace_ids {
        let params = BacktraceFrameParams {
            backtrace_id: *backtrace_id,
        };
        let raw_rows = raw_stmt
            .facet_query_ref::<StoredBacktraceFrameRow, _>(&params)
            .map_err(|error| format!("query backtrace_frames: {error}"))?;
        if raw_rows.is_empty() {
            return Err(format!(
                "invariant violated: referenced backtrace {} missing in storage",
                backtrace_id
            ));
        }
        let owner_process_id = raw_rows[0].process_id.clone();
        if raw_rows
            .iter()
            .any(|row| row.process_id != owner_process_id)
        {
            return Err(format!(
                "invariant violated: backtrace {} spans multiple process_id values in backtrace_frames",
                backtrace_id
            ));
        }

        let symbolicated_by_index = symbol_stmt
            .facet_query_ref::<SymbolicatedFrameRow, _>(&params)
            .map_err(|error| format!("query symbolicated_frames: {error}"))?
            .into_iter()
            .map(|row| {
                if row.process_id != owner_process_id {
                    return Err(format!(
                        "invariant violated: symbolicated row for backtrace {} has mismatched process_id '{}' (expected '{}')",
                        backtrace_id,
                        row.process_id.as_str(),
                        owner_process_id.as_str()
                    ));
                }
                Ok(row)
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .map(|row| (row.frame_index, row))
            .collect();

        batches.push(BacktraceFrameBatch {
            backtrace_id: *backtrace_id,
            raw_rows,
            symbolicated_by_index,
        });
    }

    Ok(batches)
}
