use std::path::{Path, PathBuf};

use rusqlite::Connection;

mod persist;
mod query;
mod schema;

pub use persist::{
    BacktraceFramePersist, StoredModuleManifestEntry, backtrace_frames_for_store,
    into_stored_module_manifest, persist_backtrace_record, persist_connection_closed,
    persist_connection_module_manifest, persist_connection_upsert, persist_cut_ack,
    persist_cut_request, persist_delta_batch,
};
pub use query::{fetch_scope_entity_links_blocking, query_named_blocking, sql_query_blocking};
pub use schema::{init_sqlite, load_next_connection_id};

#[derive(Debug, Clone)]
pub struct Db {
    path: PathBuf,
}

impl Db {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.path).map_err(|error| format!("open sqlite: {error}"))
    }
}
