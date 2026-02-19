use super::*;
use super::db::{runtime_db, runtime_stream_id, RuntimeDb, EdgeKey};
use compact_str::CompactString;
use peeps_types::{EdgeKind, EntityBody, EntityId, LockKind, ScopeBody, ScopeId};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

struct PendingOnceThenReady {
    pending: bool,
}

impl Future for PendingOnceThenReady {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.pending {
            self.pending = false;
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

struct AlwaysPending;

impl Future for AlwaysPending {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<StdMutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| StdMutex::new(()))
        .lock()
        .expect("test guard mutex poisoned")
}

fn reset_runtime_db_for_test() {
    let mut db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    *db = RuntimeDb::new(runtime_stream_id(), MAX_EVENTS);
    HELD_MUTEX_STACK.with(|stack| stack.borrow_mut().clear());
}

fn edge_exists(src: &EntityId, dst: &EntityId, kind: EdgeKind) -> bool {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.edges.contains_key(&EdgeKey {
        src: EntityId::new(src.as_str()),
        dst: EntityId::new(dst.as_str()),
        kind,
    })
}

fn edge_exists_any(src: &EntityId, dst: &EntityId) -> bool {
    edge_exists(src, dst, EdgeKind::Needs) || edge_exists(src, dst, EdgeKind::Polls)
}

fn entity_exists(id: &EntityId) -> bool {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.entities.contains_key(id)
}

fn entity_id_by_name(name: &str) -> Option<EntityId> {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.entities
        .values()
        .find(|entity| entity.name.as_str() == name)
        .map(|entity| EntityId::new(entity.id.as_str()))
}

fn entity_source_by_name(name: &str) -> Option<CompactString> {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.entities
        .values()
        .find(|entity| entity.name.as_str() == name)
        .map(|entity| entity.source.clone())
}

fn entity_has_task_scope_link(id: &EntityId) -> bool {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.entity_scope_links.keys().any(|(entity_id, scope_id)| {
        entity_id == id
            && db
                .scopes
                .get(scope_id)
                .is_some_and(|scope| matches!(&scope.body, ScopeBody::Task))
    })
}

fn entity_has_process_scope_link(id: &EntityId) -> bool {
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    db.entity_scope_links.keys().any(|(entity_id, scope_id)| {
        entity_id == id
            && db
                .scopes
                .get(scope_id)
                .is_some_and(|scope| matches!(&scope.body, ScopeBody::Process))
    })
}

fn remove_process_scope_links(id: &EntityId) {
    let mut db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    let scope_ids: Vec<ScopeId> = db
        .entity_scope_links
        .keys()
        .filter_map(|(entity_id, scope_id)| {
            if entity_id != id {
                return None;
            }
            let is_process = db
                .scopes
                .get(scope_id)
                .is_some_and(|scope| matches!(&scope.body, ScopeBody::Process));
            if is_process {
                Some(ScopeId::new(scope_id.as_str()))
            } else {
                None
            }
        })
        .collect();
    for scope_id in scope_ids {
        db.unlink_entity_from_scope(id, &scope_id);
    }
}

#[test]
fn instrument_future_named_uses_caller_source() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let marker_line = line!() + 1;
    let fut = instrument_future_named(
        "test.future.source",
        std::future::ready(()),
        Source::caller(),
    );
    let fut_id = EntityId::new(fut.future_handle.id().as_str());
    let source = {
        let db = runtime_db()
            .lock()
            .expect("runtime db lock should be available");
        db.entities
            .get(&fut_id)
            .expect("future entity should exist")
            .source
            .clone()
    };

    assert!(
        source.ends_with(&format!(":{}", marker_line)),
        "expected caller line {}, got source {}",
        marker_line,
        source
    );
}

#[test]
fn peeps_macro_tracks_caller_source() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let marker_line = line!() + 1;
    let fut = crate::peeps!(
        name = "test.future.macro_source",
        fut = std::future::ready(())
    );
    let fut_id = EntityId::new(fut.future_handle.id().as_str());
    let source = {
        let db = runtime_db()
            .lock()
            .expect("runtime db lock should be available");
        db.entities
            .get(&fut_id)
            .expect("future entity should exist")
            .source
            .clone()
    };

    assert!(
        source.ends_with(&format!(":{}", marker_line)),
        "expected caller line {}, got source {}",
        marker_line,
        source
    );
}

#[test]
fn peep_macro_records_meta_fields() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let fut = crate::peep!(
        std::future::ready(()),
        "test.future.meta_fields",
        {
            "method" => "Store.put_chunk",
            "chunk.bytes" => 42u64,
        }
    );
    let fut_id = EntityId::new(fut.future_handle.id().as_str());
    let meta = {
        let db = runtime_db()
            .lock()
            .expect("runtime db lock should be available");
        db.entities
            .get(&fut_id)
            .expect("future entity should exist")
            .meta
            .clone()
    };

    let meta_obj = meta.as_object().expect("future meta should be an object");
    assert_eq!(
        meta_obj
            .get("method")
            .and_then(|v| v.as_string())
            .map(|s| s.as_str()),
        Some("Store.put_chunk")
    );
    assert_eq!(
        meta_obj
            .get("chunk.bytes")
            .and_then(|v| v.as_number())
            .and_then(|n| n.to_u64()),
        Some(42)
    );
}

#[test]
fn instrumented_future_promotes_polls_to_needs_and_clears_on_ready() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let target = EntityHandle::new("test.target.transition", EntityBody::Future);
    let fut = instrument_future_on(
        "test.future.transition",
        &target,
        PendingOnceThenReady { pending: true },
        Source::caller(),
    );
    let fut_id = EntityId::new(fut.future_handle.id().as_str());

    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut fut = Box::pin(fut);

    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
    assert!(edge_exists(&fut_id, target.id(), EdgeKind::Needs));
    assert!(!edge_exists(&fut_id, target.id(), EdgeKind::Polls));

    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Ready(())));
    assert!(!edge_exists(&fut_id, target.id(), EdgeKind::Needs));
    assert!(!edge_exists(&fut_id, target.id(), EdgeKind::Polls));
}

#[test]
fn dropping_pending_instrumented_future_clears_edge_without_entity_teardown() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let target = EntityHandle::new("test.target.drop", EntityBody::Future);
    let fut =
        instrument_future_on("test.future.drop", &target, AlwaysPending, Source::caller());
    let fut_handle = fut.future_handle.clone();
    let fut_id = EntityId::new(fut_handle.id().as_str());

    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut fut = Box::pin(fut);

    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
    assert!(edge_exists(&fut_id, target.id(), EdgeKind::Needs));
    assert!(entity_exists(&fut_id));

    drop(fut);
    assert!(entity_exists(&fut_id));
    assert!(!edge_exists(&fut_id, target.id(), EdgeKind::Needs));
    assert!(!edge_exists(&fut_id, target.id(), EdgeKind::Polls));
}

#[test]
fn dropping_pending_operation_future_clears_needs_but_keeps_touches() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let sem = crate::semaphore!("test.semaphore.touch.resource", 0);
    let sem_id = entity_id_by_name("test.semaphore.touch.resource")
        .expect("semaphore entity should exist");

    let fut = crate::peep!(sem.acquire_owned(), "test.semaphore.touch.acquire");
    let fut_handle = fut.future_handle.clone();
    let fut_id = EntityId::new(fut_handle.id().as_str());

    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut fut = Box::pin(fut);

    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
    assert!(edge_exists(&fut_id, &sem_id, EdgeKind::Needs));
    assert!(edge_exists(&fut_id, &sem_id, EdgeKind::Touches));

    drop(fut);
    assert!(entity_exists(&fut_id));
    assert!(!edge_exists(&fut_id, &sem_id, EdgeKind::Needs));
    assert!(edge_exists(&fut_id, &sem_id, EdgeKind::Touches));
}

#[tokio::test(flavor = "current_thread")]
async fn peep_child_future_links_to_current_parent_future() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let parent = spawn_tracked("test.parent.future", async {
        crate::peep!(std::future::pending::<()>(), "test.child.future").await;
    });

    let mut found = false;
    for _ in 0..64 {
        tokio::task::yield_now().await;
        let Some(parent_id) = entity_id_by_name("test.parent.future") else {
            continue;
        };
        let Some(child_id) = entity_id_by_name("test.child.future") else {
            continue;
        };
        if edge_exists(&parent_id, &child_id, EdgeKind::Needs) {
            found = true;
            break;
        }
    }

    parent.abort();
    let _ = parent.await;

    assert!(
        found,
        "expected child future to link to parent future via needs edge"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_tracked_emits_task_scope_and_links_parent_entity() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let parent = crate::spawn_tracked!("test.task.scope.parent", async {
        crate::peep!(std::future::pending::<()>(), "test.task.scope.child").await;
    });

    let mut found = false;
    for _ in 0..64 {
        tokio::task::yield_now().await;
        let Some(parent_id) = entity_id_by_name("test.task.scope.parent") else {
            continue;
        };
        if entity_has_task_scope_link(&parent_id) {
            found = true;
            break;
        }
    }

    parent.abort();
    let _ = parent.await;

    assert!(
        found,
        "expected spawned future entity to link to a task scope"
    );
}

#[test]
fn entity_creation_always_links_process_scope() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let entity = EntityHandle::new(
        "test.process.scope.entity",
        EntityBody::Future,
        Source::caller(),
    );
    let entity_id = EntityId::new(entity.id().as_str());

    assert!(
        entity_has_process_scope_link(&entity_id),
        "expected created entity to always have process scope link"
    );
}

#[test]
fn edge_upsert_repairs_missing_process_scope_links_for_endpoints() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let src = EntityHandle::new(
        "test.process.scope.src",
        EntityBody::Future,
        Source::caller(),
    );
    let dst = EntityHandle::new(
        "test.process.scope.dst",
        EntityBody::Future,
        Source::caller(),
    );
    let src_id = EntityId::new(src.id().as_str());
    let dst_id = EntityId::new(dst.id().as_str());

    remove_process_scope_links(&src_id);
    remove_process_scope_links(&dst_id);
    assert!(!entity_has_process_scope_link(&src_id));
    assert!(!entity_has_process_scope_link(&dst_id));

    src.link_to_handle(&dst, EdgeKind::Needs);

    assert!(
        entity_has_process_scope_link(&src_id),
        "expected source process scope link to be restored during edge upsert"
    );
    assert!(
        entity_has_process_scope_link(&dst_id),
        "expected target process scope link to be restored during edge upsert"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn peep_with_on_keeps_parent_and_resource_chain() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let target = EntityHandle::new("test.resource.target", EntityBody::Future);
    let target_id = EntityId::new(target.id().as_str());
    let parent = spawn_tracked("test.parent.with_on", async move {
        crate::peeps!(
            name = "test.child.with_on",
            on = target,
            fut = std::future::pending::<()>()
        )
        .await;
    });

    let mut chain_found = false;
    for _ in 0..64 {
        tokio::task::yield_now().await;
        let Some(parent_id) = entity_id_by_name("test.parent.with_on") else {
            continue;
        };
        let Some(child_id) = entity_id_by_name("test.child.with_on") else {
            continue;
        };
        if edge_exists_any(&parent_id, &child_id) && edge_exists_any(&child_id, &target_id) {
            chain_found = true;
            break;
        }
    }

    parent.abort();
    let _ = parent.await;

    assert!(
        chain_found,
        "expected parent->child and child->target await chain edges"
    );
}

#[test]
fn mutex_creates_lock_entity() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let _lock = crate::mutex!("test.lock.entity", ());
    let lock_id = entity_id_by_name("test.lock.entity").expect("lock entity should exist");
    let db = runtime_db()
        .lock()
        .expect("runtime db lock should be available");
    let body = &db
        .entities
        .get(&lock_id)
        .expect("lock entity should be persisted")
        .body;
    match body {
        EntityBody::Lock(lock) => match &lock.kind {
            LockKind::Mutex => {}
            _ => panic!("expected mutex lock entity kind"),
        },
        _ => panic!("expected lock entity body"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contended_mutex_lock_connects_waiter_and_holder_through_lock() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let lock = Arc::new(crate::mutex!("test.lock.shared.async", ()));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let lock_for_holder = Arc::clone(&lock);
    let barrier_for_holder = Arc::clone(&barrier);
    let holder = crate::spawn_tracked!("test.lock.holder.async", async move {
        let _guard = lock_for_holder.lock();
        barrier_for_holder.wait();
        std::thread::sleep(Duration::from_millis(150));
    });

    let lock_for_waiter = Arc::clone(&lock);
    let barrier_for_waiter = Arc::clone(&barrier);
    let waiter = crate::spawn_tracked!("test.lock.waiter.async", async move {
        barrier_for_waiter.wait();
        let _guard = lock_for_waiter.lock();
    });

    let mut saw_expected_edges = false;
    for _ in 0..60 {
        let Some(holder_id) = entity_id_by_name("test.lock.holder.async") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };
        let Some(waiter_id) = entity_id_by_name("test.lock.waiter.async") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };
        let Some(lock_id) = entity_id_by_name("test.lock.shared.async") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };

        if edge_exists(&waiter_id, &lock_id, EdgeKind::Needs)
            && edge_exists(&lock_id, &holder_id, EdgeKind::Needs)
        {
            saw_expected_edges = true;
            break;
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let _ = holder.await;
    let _ = waiter.await;

    assert!(
        saw_expected_edges,
        "expected waiter->lock and lock->holder needs edges while contention is active"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contended_semaphore_connects_waiter_and_holder_through_holds_edge() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let sem = Arc::new(crate::semaphore!("test.semaphore.shared.async", 1));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let sem_for_holder = Arc::clone(&sem);
    let barrier_for_holder = Arc::clone(&barrier);
    let holder = crate::spawn_tracked!("test.semaphore.holder.async", async move {
        let _permit = sem_for_holder
            .acquire_owned()
            .await
            .expect("holder should acquire permit");
        barrier_for_holder.wait();
        std::thread::sleep(Duration::from_millis(150));
    });

    let sem_for_waiter = Arc::clone(&sem);
    let barrier_for_waiter = Arc::clone(&barrier);
    let waiter = crate::spawn_tracked!("test.semaphore.waiter.async", async move {
        barrier_for_waiter.wait();
        let _permit = crate::peep!(
            sem_for_waiter.acquire_owned(),
            "test.semaphore.waiter.acquire"
        )
        .await
        .expect("waiter should eventually acquire permit");
    });

    let mut saw_expected_edges = false;
    for _ in 0..60 {
        let Some(holder_id) = entity_id_by_name("test.semaphore.holder.async") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };
        let Some(waiter_acquire_id) = entity_id_by_name("test.semaphore.waiter.acquire") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };
        let Some(sem_id) = entity_id_by_name("test.semaphore.shared.async") else {
            tokio::time::sleep(Duration::from_millis(10)).await;
            continue;
        };

        if edge_exists(&waiter_acquire_id, &sem_id, EdgeKind::Needs)
            && edge_exists(&sem_id, &holder_id, EdgeKind::Holds)
        {
            saw_expected_edges = true;
            break;
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let _ = holder.await;
    let _ = waiter.await;

    assert!(
        saw_expected_edges,
        "expected waiter->semaphore needs edge and semaphore->holder holds edge while contention is active"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semaphore_acquire_owned_uses_caller_source() {
    let _guard = test_guard();
    reset_runtime_db_for_test();

    let sem = crate::semaphore!("test.semaphore.source", 0);
    let marker_line = line!() + 1;
    let waiter = tokio::spawn(async move {
        let _ = sem.acquire_owned().await;
    });

    let mut source = None;
    for _ in 0..64 {
        tokio::task::yield_now().await;
        source = entity_source_by_name("semaphore.acquire_owned");
        if source.is_some() {
            break;
        }
    }

    waiter.abort();
    let _ = waiter.await;

    let source = source.expect("semaphore.acquire_owned future should be tracked");
    assert!(
        source.ends_with(&format!(":{}", marker_line))
            || source.ends_with(&format!(":{}", marker_line + 1)),
        "expected caller line {} (or {}), got source {}",
        marker_line,
        marker_line + 1,
        source
    );
}
