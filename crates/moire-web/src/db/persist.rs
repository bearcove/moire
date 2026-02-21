use std::sync::Arc;

use facet::Facet;
use moire_wire::{BacktraceRecord, ModuleIdentity, ModuleManifestEntry};
use rusqlite::params;
use rusqlite_facet::{ConnectionFacetExt, StatementFacetExt};

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

#[derive(Facet)]
struct ConnectionUpsertParams {
    conn_id: u64,
    process_name: String,
    pid: u32,
    connected_at_ns: i64,
}

#[derive(Facet)]
struct ConnectionClosedParams {
    conn_id: u64,
    disconnected_at_ns: i64,
}

#[derive(Facet)]
struct ConnectionIdParams {
    conn_id: u64,
}

#[derive(Facet)]
struct ConnectionModuleInsertParams {
    conn_id: u64,
    module_index: i64,
    module_path: String,
    module_identity: String,
    arch: String,
    runtime_base: u64,
}

#[derive(Facet)]
struct BacktraceInsertParams {
    conn_id: u64,
    backtrace_id: u64,
    frame_count: i64,
    received_at_ns: i64,
}

#[derive(Facet)]
struct BacktraceFrameInsertParams {
    conn_id: u64,
    backtrace_id: u64,
    frame_index: u32,
    module_path: String,
    module_identity: String,
    rel_pc: u64,
}

#[derive(Facet)]
struct CutRequestParams {
    cut_id: String,
    requested_at_ns: i64,
}

#[derive(Facet)]
struct CutAckParams {
    cut_id: String,
    conn_id: u64,
    stream_id: String,
    next_seq_no: u64,
    received_at_ns: i64,
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
        conn.facet_execute_ref(
            "INSERT INTO connections (conn_id, process_name, pid, connected_at_ns, disconnected_at_ns)
             VALUES (:conn_id, :process_name, :pid, :connected_at_ns, NULL)
             ON CONFLICT(conn_id) DO UPDATE SET
               process_name = excluded.process_name,
               pid = excluded.pid",
            &ConnectionUpsertParams {
                conn_id,
                process_name,
                pid,
                connected_at_ns: now_nanos(),
            },
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
        conn.facet_execute_ref(
            "UPDATE connections
             SET disconnected_at_ns = :disconnected_at_ns
             WHERE conn_id = :conn_id",
            &ConnectionClosedParams {
                conn_id,
                disconnected_at_ns: now_nanos(),
            },
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
        {
            let mut delete_stmt = tx
                .prepare("DELETE FROM connection_modules WHERE conn_id = :conn_id")
                .map_err(|error| format!("prepare delete connection_modules: {error}"))?;
            delete_stmt
                .facet_execute_ref(&ConnectionIdParams { conn_id })
                .map_err(|error| format!("delete connection_modules: {error}"))?;
        }

        {
            let mut insert_stmt = tx
                .prepare(
                    "INSERT INTO connection_modules (
                        conn_id, module_index, module_path, module_identity, arch, runtime_base
                     ) VALUES (
                        :conn_id, :module_index, :module_path, :module_identity, :arch, :runtime_base
                     )",
                )
                .map_err(|error| format!("prepare insert connection_modules: {error}"))?;
            for (module_index, module) in module_manifest.iter().enumerate() {
                insert_stmt
                    .facet_execute_ref(&ConnectionModuleInsertParams {
                        conn_id,
                        module_index: module_index as i64,
                        module_path: module.module_path.clone(),
                        module_identity: module.module_identity.clone(),
                        arch: module.arch.clone(),
                        runtime_base: module.runtime_base,
                    })
                    .map_err(|error| format!("insert connection_module[{module_index}]: {error}"))?;
            }
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
        let inserted = {
            let mut insert_backtrace_stmt = tx
                .prepare(
                    "INSERT INTO backtraces (conn_id, backtrace_id, frame_count, received_at_ns)
                     VALUES (:conn_id, :backtrace_id, :frame_count, :received_at_ns)
                     ON CONFLICT(conn_id, backtrace_id) DO NOTHING",
                )
                .map_err(|error| format!("prepare insert backtrace: {error}"))?;
            insert_backtrace_stmt
                .facet_execute_ref(&BacktraceInsertParams {
                    conn_id,
                    backtrace_id,
                    frame_count: frames.len() as i64,
                    received_at_ns: now_nanos(),
                })
                .map_err(|error| format!("insert backtrace: {error}"))?
                > 0
        };
        if inserted {
            {
                let mut insert_frame_stmt = tx
                    .prepare(
                        "INSERT INTO backtrace_frames (
                            conn_id, backtrace_id, frame_index, module_path, module_identity, rel_pc
                         ) VALUES (
                            :conn_id, :backtrace_id, :frame_index, :module_path, :module_identity, :rel_pc
                         )",
                    )
                    .map_err(|error| format!("prepare insert backtrace frames: {error}"))?;
                for frame in &frames {
                    insert_frame_stmt
                        .facet_execute_ref(&BacktraceFrameInsertParams {
                            conn_id,
                            backtrace_id,
                            frame_index: frame.frame_index,
                            module_path: frame.module_path.clone(),
                            module_identity: frame.module_identity.clone(),
                            rel_pc: frame.rel_pc,
                        })
                        .map_err(|error| {
                            format!(
                                "insert backtrace frame {}/{}: {error}",
                                frame.frame_index, backtrace_id
                            )
                        })?;
                }
            }
        }
        tx.commit()
            .map_err(|error| format!("commit backtrace record: {error}"))?;
        Ok::<bool, String>(inserted)
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

pub async fn persist_cut_request(
    db: Arc<Db>,
    cut_id: String,
    requested_at_ns: i64,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = db.open()?;
        conn.facet_execute_ref(
            "INSERT INTO cuts (cut_id, requested_at_ns) VALUES (?1, ?2)
             ON CONFLICT(cut_id) DO UPDATE SET requested_at_ns = excluded.requested_at_ns",
            &CutRequestParams {
                cut_id,
                requested_at_ns,
            },
        )
        .map_err(|error| format!("upsert cut: {error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

pub async fn persist_cut_ack(
    db: Arc<Db>,
    cut_id: String,
    conn_id: u64,
    stream_id: String,
    next_seq_no: u64,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let conn = db.open()?;
        conn.facet_execute_ref(
            "INSERT INTO cut_acks (cut_id, conn_id, stream_id, next_seq_no, received_at_ns)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(cut_id, conn_id) DO UPDATE SET
               stream_id = excluded.stream_id,
               next_seq_no = excluded.next_seq_no,
               received_at_ns = excluded.received_at_ns",
            &CutAckParams {
                cut_id,
                conn_id,
                stream_id,
                next_seq_no,
                received_at_ns: now_nanos(),
            },
        )
        .map_err(|error| format!("upsert cut ack: {error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| format!("join sqlite: {error}"))?
}

pub async fn persist_delta_batch(
    db: Arc<Db>,
    conn_id: u64,
    batch: moire_types::PullChangesResponse,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || persist_delta_batch_blocking(&db, conn_id, &batch))
        .await
        .map_err(|error| format!("join sqlite: {error}"))?
}

fn persist_delta_batch_blocking(
    db: &Db,
    conn_id: u64,
    batch: &moire_types::PullChangesResponse,
) -> Result<(), String> {
    use moire_types::Change;

    let mut conn = db.open()?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("start transaction: {error}"))?;
    let stream_id = batch.stream_id.0.as_str().to_string();
    let received_at_ns = now_nanos();
    let payload_json =
        facet_json::to_string(batch).map_err(|error| format!("encode batch: {error}"))?;

    tx.execute(
        "INSERT INTO delta_batches (
            conn_id, stream_id, from_seq_no, next_seq_no, truncated,
            compacted_before_seq_no, change_count, payload_json, received_at_ns
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            to_i64_u64(conn_id),
            stream_id,
            to_i64_u64(batch.from_seq_no.0),
            to_i64_u64(batch.next_seq_no.0),
            if batch.truncated { 1_i64 } else { 0_i64 },
            batch
                .compacted_before_seq_no
                .map(|seq_no| to_i64_u64(seq_no.0)),
            to_i64_u64(batch.changes.len() as u64),
            payload_json,
            received_at_ns,
        ],
    )
    .map_err(|error| format!("insert delta batch: {error}"))?;

    for stamped in &batch.changes {
        match &stamped.change {
            Change::UpsertEntity(entity) => {
                let entity_json = facet_json::to_string(entity)
                    .map_err(|error| format!("encode entity: {error}"))?;
                tx.execute(
                    "INSERT INTO entities (conn_id, stream_id, entity_id, entity_json, updated_at_ns)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(conn_id, stream_id, entity_id) DO UPDATE SET
                       entity_json = excluded.entity_json,
                       updated_at_ns = excluded.updated_at_ns",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        entity.id.as_str(),
                        entity_json,
                        received_at_ns
                    ],
                )
                .map_err(|error| format!("upsert entity: {error}"))?;
            }
            Change::UpsertScope(scope) => {
                let scope_json = facet_json::to_string(scope)
                    .map_err(|error| format!("encode scope: {error}"))?;
                tx.execute(
                    "INSERT INTO scopes (conn_id, stream_id, scope_id, scope_json, updated_at_ns)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(conn_id, stream_id, scope_id) DO UPDATE SET
                       scope_json = excluded.scope_json,
                       updated_at_ns = excluded.updated_at_ns",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        scope.id.as_str(),
                        scope_json,
                        received_at_ns
                    ],
                )
                .map_err(|error| format!("upsert scope: {error}"))?;
            }
            Change::UpsertEntityScopeLink {
                entity_id,
                scope_id,
            } => {
                tx.execute(
                    "INSERT INTO entity_scope_links (conn_id, stream_id, entity_id, scope_id, updated_at_ns)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(conn_id, stream_id, entity_id, scope_id) DO UPDATE SET
                       updated_at_ns = excluded.updated_at_ns",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        entity_id.as_str(),
                        scope_id.as_str(),
                        received_at_ns
                    ],
                )
                .map_err(|error| format!("upsert entity_scope_link: {error}"))?;
            }
            Change::RemoveEntity { id } => {
                tx.execute(
                    "DELETE FROM entities WHERE conn_id = ?1 AND stream_id = ?2 AND entity_id = ?3",
                    params![to_i64_u64(conn_id), batch.stream_id.0.as_str(), id.as_str()],
                )
                .map_err(|error| format!("delete entity: {error}"))?;
                tx.execute(
                    "DELETE FROM entity_scope_links WHERE conn_id = ?1 AND stream_id = ?2 AND entity_id = ?3",
                    params![to_i64_u64(conn_id), batch.stream_id.0.as_str(), id.as_str()],
                )
                .map_err(|error| format!("delete entity_scope_links for entity: {error}"))?;
                tx.execute(
                    "DELETE FROM edges
                     WHERE conn_id = ?1 AND stream_id = ?2 AND (src_id = ?3 OR dst_id = ?3)",
                    params![to_i64_u64(conn_id), batch.stream_id.0.as_str(), id.as_str()],
                )
                .map_err(|error| format!("delete incident edges: {error}"))?;
            }
            Change::RemoveScope { id } => {
                tx.execute(
                    "DELETE FROM scopes WHERE conn_id = ?1 AND stream_id = ?2 AND scope_id = ?3",
                    params![to_i64_u64(conn_id), batch.stream_id.0.as_str(), id.as_str()],
                )
                .map_err(|error| format!("delete scope: {error}"))?;
                tx.execute(
                    "DELETE FROM entity_scope_links WHERE conn_id = ?1 AND stream_id = ?2 AND scope_id = ?3",
                    params![to_i64_u64(conn_id), batch.stream_id.0.as_str(), id.as_str()],
                )
                .map_err(|error| format!("delete entity_scope_links for scope: {error}"))?;
            }
            Change::RemoveEntityScopeLink {
                entity_id,
                scope_id,
            } => {
                tx.execute(
                    "DELETE FROM entity_scope_links
                     WHERE conn_id = ?1 AND stream_id = ?2 AND entity_id = ?3 AND scope_id = ?4",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        entity_id.as_str(),
                        scope_id.as_str()
                    ],
                )
                .map_err(|error| format!("delete entity_scope_link: {error}"))?;
            }
            Change::UpsertEdge(edge) => {
                let kind_json = facet_json::to_string(&edge.kind)
                    .map_err(|error| format!("encode edge kind: {error}"))?;
                let edge_json =
                    facet_json::to_string(edge).map_err(|error| format!("encode edge: {error}"))?;
                tx.execute(
                    "INSERT INTO edges (conn_id, stream_id, src_id, dst_id, kind_json, edge_json, updated_at_ns)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(conn_id, stream_id, src_id, dst_id, kind_json) DO UPDATE SET
                       edge_json = excluded.edge_json,
                       updated_at_ns = excluded.updated_at_ns",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        edge.src.as_str(),
                        edge.dst.as_str(),
                        kind_json,
                        edge_json,
                        received_at_ns
                    ],
                )
                .map_err(|error| format!("upsert edge: {error}"))?;
            }
            Change::RemoveEdge { src, dst, kind } => {
                let kind_json = facet_json::to_string(kind)
                    .map_err(|error| format!("encode edge kind: {error}"))?;
                tx.execute(
                    "DELETE FROM edges
                     WHERE conn_id = ?1 AND stream_id = ?2 AND src_id = ?3 AND dst_id = ?4 AND kind_json = ?5",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        src.as_str(),
                        dst.as_str(),
                        kind_json
                    ],
                )
                .map_err(|error| format!("delete edge: {error}"))?;
            }
            Change::AppendEvent(event) => {
                let event_json = facet_json::to_string(event)
                    .map_err(|error| format!("encode event: {error}"))?;
                tx.execute(
                    "INSERT OR REPLACE INTO events (conn_id, stream_id, seq_no, event_id, event_json, at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        to_i64_u64(conn_id),
                        batch.stream_id.0.as_str(),
                        to_i64_u64(stamped.seq_no.0),
                        event.id.as_str(),
                        event_json,
                        to_i64_u64(event.at.as_millis()),
                    ],
                )
                .map_err(|error| format!("append event: {error}"))?;
            }
        }
    }

    tx.execute(
        "INSERT INTO stream_cursors (conn_id, stream_id, next_seq_no, updated_at_ns)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(conn_id, stream_id) DO UPDATE SET
           next_seq_no = excluded.next_seq_no,
           updated_at_ns = excluded.updated_at_ns",
        params![
            to_i64_u64(conn_id),
            batch.stream_id.0.as_str(),
            to_i64_u64(batch.next_seq_no.0),
            received_at_ns
        ],
    )
    .map_err(|error| format!("upsert stream cursor: {error}"))?;

    tx.commit()
        .map_err(|error| format!("commit transaction: {error}"))?;
    Ok(())
}
