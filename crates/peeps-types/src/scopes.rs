use compact_str::CompactString;
use facet::Facet;

use crate::{caller_source, next_scope_id, MetaSerializeError, PTime, ScopeId};

/// A scope groups execution context over time (for example process/thread/task).
#[derive(Facet)]
pub struct Scope {
    /// Opaque scope identifier.
    pub id: ScopeId,

    /// When we first started tracking this scope.
    pub birth: PTime,

    /// Creation/discovery site in source code as `{path}:{line}`.
    pub source: CompactString,

    /// Human-facing name for this scope.
    pub name: CompactString,

    /// More specific info about the scope.
    pub body: ScopeBody,

    /// Extensible metadata for optional, non-canonical context.
    pub meta: facet_value::Value,
}

impl Scope {
    /// Starts building a scope from required semantic fields.
    pub fn builder(name: impl Into<CompactString>, body: ScopeBody) -> ScopeBuilder {
        ScopeBuilder {
            name: name.into(),
            body,
        }
    }

    /// Convenience constructor that accepts typed meta and builds immediately.
    #[track_caller]
    pub fn new<M>(
        name: impl Into<CompactString>,
        body: ScopeBody,
        meta: &M,
    ) -> Result<Self, MetaSerializeError>
    where
        M: for<'facet> Facet<'facet>,
    {
        Scope::builder(name, body).build(meta)
    }
}

/// Builder for `Scope` that auto-fills runtime identity and creation metadata.
pub struct ScopeBuilder {
    name: CompactString,
    body: ScopeBody,
}

impl ScopeBuilder {
    /// Finalizes the scope with typed meta converted into `facet_value::Value`.
    #[track_caller]
    pub fn build<M>(self, meta: &M) -> Result<Scope, MetaSerializeError>
    where
        M: for<'facet> Facet<'facet>,
    {
        Ok(Scope {
            id: next_scope_id(),
            birth: PTime::now(),
            name: self.name,
            source: caller_source(),
            body: self.body,
            meta: facet_value::to_value(meta)?,
        })
    }
}

#[derive(Facet)]
#[repr(u8)]
#[facet(rename_all = "snake_case")]
pub enum ScopeBody {
    Process,
    Thread,
    Task,
}
