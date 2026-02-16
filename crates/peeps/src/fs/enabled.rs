use std::io;
use std::path::Path;
use std::time::Instant;

use facet::Facet;
use peeps_types::{Node, NodeKind};

// ── Attrs struct ─────────────────────────────────────────

#[derive(Facet)]
struct FileOpAttrs {
    #[facet(rename = "fs.op")]
    fs_op: String,
    #[facet(rename = "resource.path")]
    resource_path: String,
    #[facet(rename = "write.bytes")]
    #[facet(skip_unless_truthy)]
    write_bytes: Option<u64>,
    #[facet(rename = "read.bytes")]
    #[facet(skip_unless_truthy)]
    read_bytes: Option<u64>,
    elapsed_ns: u64,
    result: String,
    #[facet(skip_unless_truthy)]
    error: Option<String>,
}

fn build_attrs_json(
    op: &str,
    path: &str,
    write_bytes: Option<u64>,
    read_bytes: Option<u64>,
    elapsed_ns: u64,
    result: &str,
    error: Option<&str>,
) -> String {
    let attrs = FileOpAttrs {
        fs_op: op.to_owned(),
        resource_path: path.to_owned(),
        write_bytes,
        read_bytes,
        elapsed_ns,
        result: result.to_owned(),
        error: error.map(|s| s.to_owned()),
    };
    facet_json::to_string(&attrs).unwrap()
}

/// Register a file_op node, emit touch edge from stack top, and return the node ID.
fn begin_op(op: &str, path: &str) -> String {
    let node_id = peeps_types::new_node_id("file_op");

    crate::registry::register_node(Node {
        id: node_id.clone(),
        kind: NodeKind::FileOp,
        label: Some(format!("{op}: {path}")),
        attrs_json: build_attrs_json(op, path, None, None, 0, "in_progress", None),
    });

    let nid = node_id.clone();
    crate::stack::with_top(|src| {
        crate::registry::touch_edge(src, &nid);
    });

    node_id
}

/// Update the file_op node with final attrs and then remove it.
fn end_op(node_id: &str, op: &str, path: &str, attrs: String) {
    crate::registry::register_node(Node {
        id: node_id.to_string(),
        kind: NodeKind::FileOp,
        label: Some(format!("{op}: {path}")),
        attrs_json: attrs,
    });
    crate::registry::remove_node(node_id);
}

pub async fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let node_id = begin_op("create_dir_all", &path_str);
    let start = Instant::now();

    let result = tokio::fs::create_dir_all(&path_buf).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(()) => ("ok", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "create_dir_all",
        &path_str,
        None,
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "create_dir_all", &path_str, attrs);

    result
}

pub async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let write_bytes = contents.as_ref().len() as u64;
    let node_id = begin_op("write", &path_str);
    let start = Instant::now();

    let result = tokio::fs::write(&path_buf, contents).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(()) => ("ok", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "write",
        &path_str,
        Some(write_bytes),
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "write", &path_str, attrs);

    result
}

pub async fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let node_id = begin_op("read_to_string", &path_str);
    let start = Instant::now();

    let result = tokio::fs::read_to_string(&path_buf).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, read_bytes, error) = match &result {
        Ok(s) => ("ok", Some(s.len() as u64), None),
        Err(e) => ("error", None, Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "read_to_string",
        &path_str,
        None,
        read_bytes,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "read_to_string", &path_str, attrs);

    result
}

pub async fn metadata(path: impl AsRef<Path>) -> io::Result<std::fs::Metadata> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let node_id = begin_op("metadata", &path_str);
    let start = Instant::now();

    let result = tokio::fs::metadata(&path_buf).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(_) => ("ok", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "metadata",
        &path_str,
        None,
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "metadata", &path_str, attrs);

    result
}

pub async fn set_permissions(
    path: impl AsRef<Path>,
    perm: std::fs::Permissions,
) -> io::Result<()> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let node_id = begin_op("set_permissions", &path_str);
    let start = Instant::now();

    let result = tokio::fs::set_permissions(&path_buf, perm).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(()) => ("ok", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "set_permissions",
        &path_str,
        None,
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "set_permissions", &path_str, attrs);

    result
}

pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from_buf = from.as_ref().to_path_buf();
    let to_buf = to.as_ref().to_path_buf();
    let label = format!(
        "{} -> {}",
        from_buf.to_string_lossy(),
        to_buf.to_string_lossy()
    );
    let node_id = begin_op("rename", &label);
    let start = Instant::now();

    let result = tokio::fs::rename(&from_buf, &to_buf).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(()) => ("ok", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "rename",
        &label,
        None,
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "rename", &label, attrs);

    result
}

pub async fn try_exists(path: impl AsRef<Path>) -> io::Result<bool> {
    let path_buf = path.as_ref().to_path_buf();
    let path_str = path_buf.to_string_lossy().into_owned();
    let node_id = begin_op("try_exists", &path_str);
    let start = Instant::now();

    let result = tokio::fs::try_exists(&path_buf).await;
    let elapsed_ns = start.elapsed().as_nanos() as u64;

    let (result_str, error) = match &result {
        Ok(true) => ("true", None),
        Ok(false) => ("false", None),
        Err(e) => ("error", Some(e.to_string())),
    };

    let attrs = build_attrs_json(
        "try_exists",
        &path_str,
        None,
        None,
        elapsed_ns,
        result_str,
        error.as_deref(),
    );
    end_op(&node_id, "try_exists", &path_str, attrs);

    result
}

