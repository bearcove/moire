use std::future::Future;

/// Diagnostic wrapper around `tokio::task::JoinSet`.
///
/// JoinSet is a first-class resource node so spawned tasks can be attached as
/// descendants through canonical edges.
pub struct JoinSet<T> {
    node_id: String,
    inner: tokio::task::JoinSet<T>,
}

impl<T> JoinSet<T>
where
    T: Send + 'static,
{
    pub fn new() -> Self {
        let node_id = peeps_types::new_node_id("joinset");
        #[cfg(feature = "diagnostics")]
        {
            crate::registry::register_node(peeps_types::Node {
                id: node_id.clone(),
                kind: peeps_types::NodeKind::JoinSet,
                label: Some("joinset".to_string()),
                attrs_json: "{}".to_string(),
            });
            crate::stack::with_top(|src| crate::registry::edge(src, &node_id));
        }
        Self {
            node_id,
            inner: tokio::task::JoinSet::new(),
        }
    }

    pub fn spawn<F>(&mut self, label: &'static str, future: F)
    where
        F: Future<Output = T> + Send + 'static,
    {
        let joinset_node_id = self.node_id.clone();
        self.inner.spawn(async move {
            // Build a stable root future for this joinset child and scope it under joinset.
            let child = crate::peepable(future, label);
            let scoped = crate::stack::scope(&joinset_node_id, child);
            crate::stack::ensure(scoped).await
        });
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn abort_all(&mut self) {
        self.inner.abort_all();
    }

    pub async fn join_next(&mut self) -> Option<Result<T, tokio::task::JoinError>> {
        self.inner.join_next().await
    }
}

impl<T> Default for JoinSet<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for JoinSet<T> {
    fn drop(&mut self) {
        #[cfg(feature = "diagnostics")]
        crate::registry::remove_node(&self.node_id);
    }
}
