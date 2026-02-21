use std::sync::Arc;

use moire_wire::{BacktraceRecord, ModuleIdentity, ModuleManifestEntry};
use rusqlite::params;

use crate::db::Db;
use crate::util::time::{now_nanos, to_i64_u64};

#[derive(Clone)]
pub struct BacktraceFramePersist {
    pub frame_index: u32,
    pub rel_pc: u64,
    pub module_path: String,
    pub module_identity: String,
}

#[derive(Clone)]
pub struct StoredModuleManifestEntry {
    pub module_path: String,
    pub module_identity: String,
    pub arch: String,
    pub runtime_base: u64,
}

pub fn backtrace_frames_for_store(
    module_manifest: &[StoredModuleManifestEntry],
    record: &BacktraceRecord,
) -> Result<Vec<BacktraceFramePersist>, String> {
    let mut frames = Vec::with_capacity(record.frames.len());
    for (frame_index, frame) in record.frames.iter().enumerate() {
        let module_id = frame.module_id.get();
        let module_idx = (module_id - 1) as usize;
        let Some(module) = module_manifest.get(module_idx) else {
            return Err(format!(
                "invariant violated: backtrace frame {frame_index} references module_id {module_id} (index {}), but manifest has {} entries",
                module_idx,
                module_manifest.len()
            ));
        };
        frames.push(BacktraceFramePersist {
            frame_index: frame_index as u32,
            rel_pc: frame.rel_pc,
            module_path: module.module_path.clone(),
            module_identity: module.module_identity.clone(),
        });
    }
    Ok(frames)
}

pub fn into_stored_module_manifest(
    module_manifest: Vec<ModuleManifestEntry>,
) -> Vec<StoredModuleManifestEntry> {
    module_manifest
        .into_iter()
        .map(|module| StoredModuleManifestEntry {
            module_path: module.module_path,
            module_identity: module_identity_key(&module.identity),
            arch: module.arch,
            runtime_base: module.runtime_base,
        })
        .collect()
}

fn module_identity_key(identity: &ModuleIdentity) -> String {
    match identity {
        ModuleIdentity::BuildId(build_id) => format!("build_id:{build_id}"),
        ModuleIdentity::DebugId(debug_id) => format!("debug_id:{debug_id}"),
    }
}

pub async fn persist_connection_upsert(
    db: Arc<Db>,
    conn_id: u64,
    process_name: String,
    pid: u32,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = db.open()?;
        conn.execute(
            "INSERT INTO connections (conn_id, process_name, pid, connected_at_ns, disconnected_at_ns)
             VALUES (?1, ?2, ?3, ?4, NULL)
             ON CONFLICT(conn_id) DO UPDATE SET
               process_name = excluded.process_name,
               pid = excluded.pid",
            params![
                to_i64_u64(conn_id),
                process_name,
                i64::from(pid),
                now_nanos()
            ],
        )
        .map_err(|error| format!("upsert connection: {error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

pub async fn persist_connection_closed(db: Arc<Db>, conn_id: u64) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = db.open()?;
        conn.execute(
            "UPDATE connections SET disconnected_at_ns = ?2 WHERE conn_id = ?1",
            params![to_i64_u64(conn_id), now_nanos()],
        )
        .map_err(|error| format!("close connection: {error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

pub async fn persist_connection_module_manifest(
    db: Arc<Db>,
    conn_id: u64,
    module_manifest: Vec<StoredModuleManifestEntry>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let mut conn = db.open()?;
        let tx = conn
            .transaction()
            .map_err(|error| format!("start transaction: {error}"))?;
        tx.execute(
            "DELETE FROM connection_modules WHERE conn_id = ?1",
            params![to_i64_u64(conn_id)],
        )
        .map_err(|error| format!("delete connection_modules: {error}"))?;

        for (module_index, module) in module_manifest.iter().enumerate() {
            tx.execute(
                "INSERT INTO connection_modules (
                    conn_id, module_index, module_path, module_identity, arch, runtime_base
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    to_i64_u64(conn_id),
                    module_index as i64,
                    module.module_path.as_str(),
                    module.module_identity.as_str(),
                    module.arch.as_str(),
                    to_i64_u64(module.runtime_base),
                ],
            )
            .map_err(|error| format!("insert connection_module[{module_index}]: {error}"))?;
        }
        tx.commit()
            .map_err(|error| format!("commit connection_modules: {error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

// r[impl symbolicate.server-store]
pub async fn persist_backtrace_record(
    db: Arc<Db>,
    conn_id: u64,
    backtrace_id: u64,
    frames: Vec<BacktraceFramePersist>,
) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        let mut conn = db.open()?;
        let tx = conn
            .transaction()
            .map_err(|error| format!("start transaction: {error}"))?;
        let inserted = tx
            .execute(
                "INSERT INTO backtraces (conn_id, backtrace_id, frame_count, received_at_ns)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(conn_id, backtrace_id) DO NOTHING",
                params![
                    to_i64_u64(conn_id),
                    to_i64_u64(backtrace_id),
                    frames.len() as i64,
                    now_nanos()
                ],
            )
            .map_err(|error| format!("insert backtrace: {error}"))?
            > 0;
        if inserted {
            for frame in &frames {
                tx.execute(
                    "INSERT INTO backtrace_frames (
                        conn_id, backtrace_id, frame_index, module_path, module_identity, rel_pc
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        to_i64_u64(conn_id),
                        to_i64_u64(backtrace_id),
                        i64::from(frame.frame_index),
                        frame.module_path.as_str(),
                        frame.module_identity.as_str(),
                        to_i64_u64(frame.rel_pc),
                    ],
                )
                .map_err(|error| {
                    format!(
                        "insert backtrace frame {}/{}: {error}",
                        frame.frame_index, backtrace_id
                    )
                })?;
            }
        }
        tx.commit()
            .map_err(|error| format!("commit backtrace record: {error}"))?;
        Ok::<bool, String>(inserted)
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}
