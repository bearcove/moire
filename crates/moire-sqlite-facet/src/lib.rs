//! SQLite projection helpers for `moire-types` snapshots.
//!
//! This crate intentionally handles the canonical four snapshot tables:
//! `entities`, `scopes`, `edges`, and `events`.
//!
//! Additional query-oriented relationship tables (for example entity<->scope
//! membership/link tables) should stay normalized at the SQLite layer instead
//! of being modeled as arrays in JSON fields.

use moire_types::Snapshot;
use rusqlite::types::{Value as SqlValue, ValueRef};

pub fn sqlite_value_ref_to_facet(value: ValueRef<'_>) -> facet_value::Value {
    match value {
        ValueRef::Null => facet_value::Value::NULL,
        ValueRef::Integer(v) => v.into(),
        ValueRef::Real(v) => v.into(),
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned().into(),
        ValueRef::Blob(bytes) => bytes.to_vec().into(),
    }
}

pub fn sqlite_value_to_facet(value: SqlValue) -> facet_value::Value {
    match value {
        SqlValue::Null => facet_value::Value::NULL,
        SqlValue::Integer(v) => v.into(),
        SqlValue::Real(v) => v.into(),
        SqlValue::Text(v) => v.into(),
        SqlValue::Blob(v) => v.into(),
    }
}

pub fn row_to_facet_array(row: &rusqlite::Row<'_>) -> rusqlite::Result<facet_value::Value> {
    let mut out = Vec::new();
    for index in 0..row.as_ref().column_count() {
        let value = row.get_ref(index)?;
        out.push(sqlite_value_ref_to_facet(value));
    }
    Ok(out.into_iter().collect())
}

pub fn facet_to_json_text(value: &facet_value::Value) -> Result<String, String> {
    facet_json::to_string(value).map_err(|e| e.to_string())
}

pub fn json_text_to_facet(text: &str) -> Result<facet_value::Value, String> {
    facet_json::from_str(text).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
pub struct SnapshotTableNames {
    // Core snapshot projection tables.
    // Keep this focused on canonical model rows; relationship/index helper
    // tables (entity_scope_links, etc.) are managed separately by callers.
    pub entities: String,
    pub scopes: String,
    pub edges: String,
    pub events: String,
}

impl Default for SnapshotTableNames {
    fn default() -> Self {
        Self {
            entities: String::from("entities"),
            scopes: String::from("scopes"),
            edges: String::from("edges"),
            events: String::from("events"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertMode {
    Insert,
    InsertOrReplace,
}

impl InsertMode {
    fn verb(self) -> &'static str {
        match self {
            Self::Insert => "INSERT INTO",
            Self::InsertOrReplace => "INSERT OR REPLACE INTO",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InsertCounts {
    pub entities: usize,
    pub scopes: usize,
    pub edges: usize,
    pub events: usize,
}

pub fn insert_snapshot_batch(
    conn: &mut rusqlite::Connection,
    snapshot_id: i64,
    snapshot: &Snapshot,
    tables: &SnapshotTableNames,
    mode: InsertMode,
) -> rusqlite::Result<InsertCounts> {
    let tx = conn.transaction()?;
    let mut counts = InsertCounts::default();

    let entities_sql = format!(
        "{} [{}] (snapshot_id, id, birth_ms, source_id, name, body_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        mode.verb(),
        tables.entities.as_str()
    );
    let scopes_sql = format!(
        "{} [{}] (snapshot_id, id, birth_ms, source_id, name, body_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        mode.verb(),
        tables.scopes.as_str()
    );
    let edges_sql = format!(
        "{} [{}] (snapshot_id, src_id, dst_id, source_id, kind_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        mode.verb(),
        tables.edges.as_str()
    );
    let events_sql = format!(
        "{} [{}] (snapshot_id, id, at_ms, source_id, target_json, kind_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        mode.verb(),
        tables.events.as_str()
    );

    {
        let mut stmt = tx.prepare_cached(&entities_sql)?;
        for entity in &snapshot.entities {
            stmt.execute(rusqlite::params![
                snapshot_id,
                entity.id,
                entity.birth,
                entity.source,
                entity.name.as_str(),
                entity.body,
            ])?;
            counts.entities += 1;
        }
    }

    {
        let mut stmt = tx.prepare_cached(&scopes_sql)?;
        for scope in &snapshot.scopes {
            stmt.execute(rusqlite::params![
                snapshot_id,
                scope.id,
                scope.birth,
                scope.source,
                scope.name.as_str(),
                scope.body,
            ])?;
            counts.scopes += 1;
        }
    }

    {
        let mut stmt = tx.prepare_cached(&edges_sql)?;
        for edge in &snapshot.edges {
            stmt.execute(rusqlite::params![
                snapshot_id,
                edge.src,
                edge.dst,
                edge.source,
                edge.kind,
            ])?;
            counts.edges += 1;
        }
    }

    {
        let mut stmt = tx.prepare_cached(&events_sql)?;
        for event in &snapshot.events {
            stmt.execute(rusqlite::params![
                snapshot_id,
                event.id,
                event.at,
                event.source,
                event.target,
                event.kind,
            ])?;
            counts.events += 1;
        }
    }

    tx.commit()?;
    Ok(counts)
}

pub fn insert_snapshot_batch_default(
    conn: &mut rusqlite::Connection,
    snapshot_id: i64,
    snapshot: &Snapshot,
) -> rusqlite::Result<InsertCounts> {
    insert_snapshot_batch(
        conn,
        snapshot_id,
        snapshot,
        &SnapshotTableNames::default(),
        InsertMode::InsertOrReplace,
    )
}
