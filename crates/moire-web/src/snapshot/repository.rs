use std::collections::BTreeMap;

use rusqlite::params;

use crate::db::Db;
use crate::util::time::to_i64_u64;

#[derive(Clone)]
pub(crate) struct StoredBacktraceFrameRow {
    pub(crate) frame_index: u32,
    pub(crate) module_path: String,
    pub(crate) module_identity: String,
    pub(crate) rel_pc: u64,
}

#[derive(Clone)]
pub(crate) struct SymbolicatedFrameRow {
    pub(crate) module_path: String,
    pub(crate) rel_pc: u64,
    pub(crate) status: String,
    pub(crate) function_name: Option<String>,
    pub(crate) source_file_path: Option<String>,
    pub(crate) source_line: Option<i64>,
    pub(crate) unresolved_reason: Option<String>,
}

pub(crate) struct BacktraceFrameBatch {
    pub(crate) backtrace_id: u64,
    pub(crate) raw_rows: Vec<StoredBacktraceFrameRow>,
    pub(crate) symbolicated_by_index: BTreeMap<u32, SymbolicatedFrameRow>,
}

pub(crate) fn load_backtrace_frame_batches(
    db: &Db,
    pairs: &[(u64, u64)],
) -> Result<Vec<BacktraceFrameBatch>, String> {
    let conn = db.open()?;

    let mut backtrace_owner: BTreeMap<u64, u64> = BTreeMap::new();
    for (conn_id, backtrace_id) in pairs {
        match backtrace_owner.insert(*backtrace_id, *conn_id) {
            None => {}
            Some(existing_conn_id) if existing_conn_id == *conn_id => {}
            Some(existing_conn_id) => {
                return Err(format!(
                    "invariant violated: backtrace_id {backtrace_id} appears on multiple connections ({existing_conn_id}, {conn_id})"
                ));
            }
        }
    }

    let mut raw_stmt = conn
        .prepare(
            "SELECT frame_index, module_path, module_identity, rel_pc
             FROM backtrace_frames
             WHERE conn_id = ?1 AND backtrace_id = ?2
             ORDER BY frame_index ASC",
        )
        .map_err(|error| format!("prepare backtrace_frames read: {error}"))?;
    let mut symbol_stmt = conn
        .prepare(
            "SELECT frame_index, module_path, rel_pc, status, function_name, source_file_path, source_line, unresolved_reason
             FROM symbolicated_frames
             WHERE conn_id = ?1 AND backtrace_id = ?2",
        )
        .map_err(|error| format!("prepare symbolicated_frames read: {error}"))?;

    let mut batches = Vec::with_capacity(backtrace_owner.len());
    for (backtrace_id, conn_id) in backtrace_owner {
        let raw_rows = raw_stmt
            .query_map(
                params![to_i64_u64(conn_id), to_i64_u64(backtrace_id)],
                |row| {
                    Ok(StoredBacktraceFrameRow {
                        frame_index: row.get::<_, i64>(0)? as u32,
                        module_path: row.get(1)?,
                        module_identity: row.get(2)?,
                        rel_pc: row.get::<_, i64>(3)? as u64,
                    })
                },
            )
            .map_err(|error| format!("query backtrace_frames: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read backtrace_frames row: {error}"))?;
        if raw_rows.is_empty() {
            return Err(format!(
                "invariant violated: referenced backtrace {backtrace_id} missing in storage"
            ));
        }

        let symbolicated_by_index = symbol_stmt
            .query_map(
                params![to_i64_u64(conn_id), to_i64_u64(backtrace_id)],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u32,
                        SymbolicatedFrameRow {
                            module_path: row.get(1)?,
                            rel_pc: row.get::<_, i64>(2)? as u64,
                            status: row.get(3)?,
                            function_name: row.get(4)?,
                            source_file_path: row.get(5)?,
                            source_line: row.get(6)?,
                            unresolved_reason: row.get(7)?,
                        },
                    ))
                },
            )
            .map_err(|error| format!("query symbolicated_frames: {error}"))?
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(|error| format!("read symbolicated_frames row: {error}"))?;

        batches.push(BacktraceFrameBatch {
            backtrace_id,
            raw_rows,
            symbolicated_by_index,
        });
    }

    Ok(batches)
}
