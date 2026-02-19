use compact_str::{CompactString, ToCompactString};
use peeps_types::infer_krate_from_source_with_manifest_dir;
use peeps_types::set_inference_source_root;
use peeps_types::{EntityBody, EntityId, Event, ScopeBody, ScopeId};
use std::cell::RefCell;
use std::future::Future;
use std::panic::Location;
use std::path::PathBuf;
use std::sync::OnceLock;

pub(super) const MAX_EVENTS: usize = 16_384;
pub(super) const MAX_CHANGES_BEFORE_COMPACT: usize = 65_536;
pub(super) const COMPACT_TARGET_CHANGES: usize = 8_192;
pub(super) const DEFAULT_STREAM_ID_PREFIX: &str = "proc";
pub(super) const DASHBOARD_PUSH_MAX_CHANGES: u32 = 2048;
pub(super) const DASHBOARD_PUSH_INTERVAL_MS: u64 = 100;
pub(super) const DASHBOARD_RECONNECT_DELAY_MS: u64 = 500;

tokio::task_local! {
    pub(super) static FUTURE_CAUSAL_STACK: RefCell<Vec<EntityId>>;
}
thread_local! {
    pub(super) static HELD_MUTEX_STACK: RefCell<Vec<EntityId>> = const { RefCell::new(Vec::new()) };
}

mod api;
pub mod channels;
mod dashboard;
pub(super) mod db;
pub mod futures;
pub mod handles;
pub mod process;
pub mod rpc;
pub mod sync;

pub use self::api::*;
pub use self::channels::*;
pub use self::futures::*;
pub use self::handles::*;
pub use self::process::*;
pub use self::rpc::*;
pub use self::sync::*;

// This identifies where something happened, for example, where a future was polled, where a task
// was created, etc.
//
// If all we were using all the time, including future poll, were macros, we could use the `file!()`
// macro and the `line!()` macro to get accurate data.
//
// But it's not, there are functions, so we have to rely on `Location::caller()` (which is what we
// encapsulate here). The problem with `Location::caller()` is that it sometimes has relative paths
// for files, like `src/main.rs` — which crate is that from? Good luck.
//
// Therefore, we ideally need two things: `$CARGO_MANIFEST_DIR` and `Location::caller()`. The latter
// is usually either grabbed just in time if all of the ancestors were marked with the #[track_caller]
// attribute. As for the former, it is registered once per process inside of the process scope.
#[derive(Clone, Copy, Debug)]
pub struct Source {
    location: &'static Location<'static>,
}

impl Source {
    #[track_caller]
    pub fn caller() -> Self {
        Self {
            location: Location::caller(),
        }
    }

    pub fn into_compact_string(self) -> CompactString {
        CompactString::from(format!("{}:{}", self.location.file(), self.location.line()))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PeepsContext {
    manifest_dir: &'static str,
}

impl PeepsContext {
    pub const fn new(manifest_dir: &'static str) -> Self {
        Self { manifest_dir }
    }

    #[track_caller]
    pub const fn caller(manifest_dir: &'static str) -> Self {
        Self::new(manifest_dir)
    }

    pub const fn manifest_dir(self) -> &'static str {
        self.manifest_dir
    }
}

static PROCESS_SCOPE: OnceLock<ScopeHandle> = OnceLock::new();

// facade! expands to a call to this
#[doc(hidden)]
pub fn __init_from_macro(manifest_dir: &str) {
    set_inference_source_root(PathBuf::from(manifest_dir));
    let process_name = std::env::current_exe()
        .unwrap()
        .display()
        .to_compact_string();
    PROCESS_SCOPE.get_or_init(|| {
        ScopeHandle::new(process_name.clone(), ScopeBody::Process, Source::caller())
    });
    dashboard::init_dashboard_push_loop(&process_name);
}

pub(super) fn current_process_scope_id() -> Option<ScopeId> {
    PROCESS_SCOPE
        .get()
        .map(|scope| ScopeId::new(scope.id().as_str()))
}

pub(super) fn current_tokio_task_key() -> Option<CompactString> {
    tokio::task::try_id().map(|id| CompactString::from(id.to_string()))
}

pub(super) struct TaskScopeRegistration {
    task_key: CompactString,
    scope: ScopeHandle,
}

impl Drop for TaskScopeRegistration {
    fn drop(&mut self) {
        if let Ok(mut db) = db::runtime_db().lock() {
            db.unregister_task_scope_id(&self.task_key, self.scope.id());
        }
    }
}

pub(super) fn register_current_task_scope(
    task_name: &str,
    source: Source,
) -> Option<TaskScopeRegistration> {
    let task_key = current_tokio_task_key()?;
    let scope = ScopeHandle::new(
        format!("task.{task_name}#{task_key}"),
        ScopeBody::Task,
        source,
    );
    if let Ok(mut db) = db::runtime_db().lock() {
        db.register_task_scope_id(&task_key, scope.id());
    }
    Some(TaskScopeRegistration { task_key, scope })
}

#[track_caller]
pub fn spawn_tracked<F>(
    name: impl Into<CompactString>,
    fut: F,
    source: Source,
) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let name: CompactString = name.into();
    tokio::spawn(
        FUTURE_CAUSAL_STACK.scope(RefCell::new(Vec::new()), async move {
            let _task_scope = register_current_task_scope(name.as_str(), source);
            instrument_future_named(name, fut, source).await
        }),
    )
}

#[track_caller]
pub fn spawn_blocking_tracked<F, T>(
    name: impl Into<CompactString>,
    f: F,
    source: Source,
) -> tokio::task::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let handle = EntityHandle::new(name, EntityBody::Future, source);
    tokio::task::spawn_blocking(move || {
        let _hold = handle;
        f()
    })
}

pub(super) fn record_event_with_source(mut event: Event, source: Source, cx: PeepsContext) {
    event.source = source.into_compact_string();
    event.krate =
        infer_krate_from_source_with_manifest_dir(event.source.as_str(), Some(cx.manifest_dir()));
    if let Ok(mut db) = db::runtime_db().lock() {
        db.record_event(event);
    }
}

pub(super) fn record_event_with_entity_source(mut event: Event, entity_id: &EntityId) {
    if let Ok(mut db) = db::runtime_db().lock() {
        if let Some(entity) = db.entities.get(entity_id) {
            event.source = CompactString::from(entity.source.as_str());
            event.krate = entity
                .krate
                .as_ref()
                .map(|k| CompactString::from(k.as_str()));
        }
        db.record_event(event);
    }
}
