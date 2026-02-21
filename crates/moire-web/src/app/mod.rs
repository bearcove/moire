use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use crate::db::{Db, StoredModuleManifestEntry};
use crate::recording::session::RecordingState;
use moire_types::SnapshotCutResponse;
use moire_wire::SnapshotReply;
use tokio::sync::{Mutex, Notify, mpsc};

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Mutex<ServerState>>,
    pub db: Arc<Db>,
    pub dev_proxy: Option<DevProxyState>,
}

#[derive(Clone)]
pub struct DevProxyState {
    pub base_url: Arc<String>,
}

pub struct ServerState {
    pub next_conn_id: u64,
    pub next_cut_id: u64,
    pub next_snapshot_id: i64,
    pub next_session_id: u64,
    pub connections: HashMap<u64, ConnectedProcess>,
    pub cuts: BTreeMap<String, CutState>,
    pub pending_snapshots: HashMap<i64, SnapshotPending>,
    pub snapshot_streams: HashMap<i64, SnapshotStreamState>,
    pub last_snapshot_json: Option<String>,
    pub recording: Option<RecordingState>,
}

pub struct ConnectedProcess {
    pub process_name: String,
    pub pid: u32,
    pub handshake_received: bool,
    pub module_manifest: Vec<StoredModuleManifestEntry>,
    pub tx: mpsc::Sender<Vec<u8>>,
}

pub struct CutState {
    pub requested_at_ns: i64,
    pub pending_conn_ids: BTreeSet<u64>,
    pub acks: BTreeMap<u64, moire_types::CutAck>,
}

pub struct SnapshotPending {
    pub pending_conn_ids: BTreeSet<u64>,
    pub replies: HashMap<u64, SnapshotReply>,
    pub notify: Arc<Notify>,
}

pub struct SnapshotStreamState {
    pub pairs: Vec<(u64, u64)>,
}

pub async fn remember_snapshot(state: &AppState, snapshot: &SnapshotCutResponse) {
    let Ok(json) = facet_json::to_string(snapshot) else {
        tracing::warn!("failed to serialize snapshot for cache");
        return;
    };
    let mut guard = state.inner.lock().await;
    guard.last_snapshot_json = Some(json);
}
