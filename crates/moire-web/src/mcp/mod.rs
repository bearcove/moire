use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::State;
use axum::http::{HeaderMap, Method, Uri};
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router};
use moire_trace_types::FrameId;
use moire_types::{
    ConnectedProcessInfo, ConnectionsResponse, CutId, CutStatusResponse,
    SourcePreviewBatchResponse, TriggerCutResponse,
};
use moire_wire::{ServerMessage, encode_server_message_default};
use rust_mcp_sdk::id_generator::{FastIdGenerator, UuidGenerator};
use rust_mcp_sdk::macros::{JsonSchema, mcp_tool};
use rust_mcp_sdk::mcp_http::{GenericBody, McpAppState, McpHttpHandler};
use rust_mcp_sdk::mcp_server::error::TransportServerError;
use rust_mcp_sdk::mcp_server::{ServerHandler, ToMcpServerHandler};
use rust_mcp_sdk::schema::{
    CallToolError, CallToolRequestParams, CallToolResult, Implementation, InitializeResult,
    LATEST_PROTOCOL_VERSION, ListToolsResult, PaginatedRequestParams, RpcError, ServerCapabilities,
    ServerCapabilitiesTools,
};
use rust_mcp_sdk::session_store::InMemorySessionStore;
use rust_mcp_sdk::{TransportOptions, tool_box};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::api::snapshot::take_snapshot_internal;
use crate::api::source::lookup_source_in_db;
use crate::app::{AppState, ConnectionId, CutState};
use crate::db::{persist_cut_request, query_named_blocking, sql_query_blocking};
use crate::snapshot::table::lookup_frame_source_by_raw;
use crate::util::time::now_nanos;

const DEFAULT_MCP_ENDPOINT: &str = "/mcp";
const DEFAULT_MCP_PING_INTERVAL: Duration = Duration::from_secs(12);

#[mcp_tool(
    name = "moire_connections",
    description = "List currently connected moire processes (conn_id, process_id, process_name, pid)."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ConnectionsTool {}

#[mcp_tool(
    name = "moire_trigger_cut",
    description = "Trigger a coordinated cut across all current connections and return cut_id plus requested connection count."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TriggerCutTool {}

#[mcp_tool(
    name = "moire_cut_status",
    description = "Get status for a specific cut_id, including pending and acked connection counts."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CutStatusTool {
    pub cut_id: String,
}

#[mcp_tool(
    name = "moire_snapshot",
    description = "Capture a fresh cross-process snapshot now (same as POST /api/snapshot)."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SnapshotTool {}

#[mcp_tool(
    name = "moire_snapshot_current",
    description = "Return the most recently cached snapshot (same as GET /api/snapshot/current)."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SnapshotCurrentTool {}

#[mcp_tool(
    name = "moire_sql_readonly",
    description = "Run a read-only SQL query against moire-web SQLite and return columns + rows."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SqlReadonlyTool {
    pub sql: String,
}

#[mcp_tool(
    name = "moire_query_pack",
    description = "Run a named query pack (blockers, blocked-senders, blocked-receivers, stalled-sends, channel-pressure, channel-health, scope-membership, missing-scope-links, stale-blockers)."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryPackTool {
    pub name: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[mcp_tool(
    name = "moire_source_preview",
    description = "Fetch source context for a single frame_id (includes context_html, compact_context_html, context_line, and enclosing_fn when available)."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SourcePreviewTool {
    pub frame_id: u64,
}

#[mcp_tool(
    name = "moire_source_previews",
    description = "Batch-fetch source context for many frame_ids. Returns previews and unavailable_frame_ids."
)]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SourcePreviewsTool {
    pub frame_ids: Vec<u64>,
}

tool_box!(
    MoireTools,
    [
        ConnectionsTool,
        TriggerCutTool,
        CutStatusTool,
        SnapshotTool,
        SnapshotCurrentTool,
        SqlReadonlyTool,
        QueryPackTool,
        SourcePreviewTool,
        SourcePreviewsTool
    ]
);

#[derive(Clone)]
struct MoireMcpHandler {
    state: AppState,
}

impl MoireMcpHandler {
    fn new(state: AppState) -> Self {
        Self { state }
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        args: &JsonMap<String, JsonValue>,
    ) -> Result<String, String> {
        match tool_name {
            "moire_connections" => self.tool_connections().await,
            "moire_trigger_cut" => self.tool_trigger_cut().await,
            "moire_cut_status" => {
                let cut_id = required_non_empty_string(args, "cut_id")?;
                self.tool_cut_status(cut_id).await
            }
            "moire_snapshot" => self.tool_snapshot().await,
            "moire_snapshot_current" => self.tool_snapshot_current().await,
            "moire_sql_readonly" => {
                let sql = required_non_empty_string(args, "sql")?;
                self.tool_sql_readonly(sql).await
            }
            "moire_query_pack" => {
                let name = required_non_empty_string(args, "name")?;
                let limit = optional_u32(args, "limit")?;
                self.tool_query_pack(name, limit).await
            }
            "moire_source_preview" => {
                let frame_id = required_u64(args, "frame_id")?;
                self.tool_source_preview(frame_id).await
            }
            "moire_source_previews" => {
                let frame_ids = required_u64_list(args, "frame_ids")?;
                self.tool_source_previews(frame_ids).await
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    async fn tool_connections(&self) -> Result<String, String> {
        let guard = self.state.inner.lock().await;
        let mut processes: Vec<ConnectedProcessInfo> = guard
            .connections
            .iter()
            .filter_map(|(conn_id, conn)| {
                let process_id = conn.process_id.clone()?;
                Some(ConnectedProcessInfo {
                    conn_id: *conn_id,
                    process_id,
                    process_name: conn.process_name.clone(),
                    pid: conn.pid,
                })
            })
            .collect();
        processes.sort_by(|a, b| {
            a.process_name
                .cmp(&b.process_name)
                .then_with(|| a.pid.cmp(&b.pid))
                .then_with(|| a.conn_id.cmp(&b.conn_id))
        });

        to_pretty_json(&ConnectionsResponse {
            connected_processes: processes.len(),
            processes,
        })
    }

    async fn tool_trigger_cut(&self) -> Result<String, String> {
        let (cut_id, cut_id_string, now_ns, requested_connections, outbound) = {
            let mut guard = self.state.inner.lock().await;
            let cut_num = guard.next_cut_id;
            guard.next_cut_id = guard.next_cut_id.next();
            let cut_id = cut_num.to_cut_id();
            let cut_id_string = cut_id.as_str().to_owned();
            let now_ns = now_nanos();
            let mut pending_conn_ids = BTreeSet::new();
            let mut outbound = Vec::new();
            for (conn_id, conn) in &guard.connections {
                pending_conn_ids.insert(*conn_id);
                outbound.push((*conn_id, conn.tx.clone()));
            }

            guard.cuts.insert(
                cut_id.clone(),
                CutState {
                    requested_at_ns: now_ns,
                    pending_conn_ids,
                    acks: BTreeMap::new(),
                },
            );

            (cut_id, cut_id_string, now_ns, outbound.len(), outbound)
        };

        let request = ServerMessage::CutRequest(moire_types::CutRequest {
            cut_id: cut_id.clone(),
        });
        if let Err(error) =
            persist_cut_request(self.state.db.clone(), cut_id_string.clone(), now_ns).await
        {
            warn!(
                %error,
                cut_id = %cut_id_string,
                "failed to persist cut request"
            );
        }
        let payload = encode_server_message_default(&request)
            .map_err(|error| format!("failed to encode cut request: {error}"))?;
        for (conn_id, tx) in outbound {
            if let Err(error) = tx.try_send(payload.clone()) {
                warn!(
                    conn_id = %conn_id,
                    %error,
                    "failed to enqueue cut request"
                );
            }
        }

        to_pretty_json(&TriggerCutResponse {
            cut_id,
            requested_at_ns: now_ns,
            requested_connections,
        })
    }

    async fn tool_cut_status(&self, cut_id_raw: String) -> Result<String, String> {
        let guard = self.state.inner.lock().await;
        let cut_id = CutId::new(cut_id_raw);
        let Some(cut) = guard.cuts.get(&cut_id) else {
            return Err(format!("unknown cut id: {}", cut_id.as_str()));
        };

        let pending_conn_ids: Vec<ConnectionId> = cut.pending_conn_ids.iter().copied().collect();
        to_pretty_json(&CutStatusResponse {
            cut_id: cut_id.clone(),
            requested_at_ns: cut.requested_at_ns,
            pending_connections: cut.pending_conn_ids.len(),
            acked_connections: cut.acks.len(),
            pending_conn_ids,
        })
    }

    async fn tool_snapshot(&self) -> Result<String, String> {
        let snapshot = take_snapshot_internal(&self.state).await;
        to_pretty_json(&snapshot)
    }

    async fn tool_snapshot_current(&self) -> Result<String, String> {
        let snapshot_json = {
            let guard = self.state.inner.lock().await;
            guard.last_snapshot_json.clone()
        };
        let Some(snapshot_json) = snapshot_json else {
            return Err("no snapshot available".to_string());
        };
        let value: facet_value::Value = facet_json::from_str(&snapshot_json)
            .map_err(|error| format!("decode cached snapshot json: {error}"))?;
        to_pretty_json(&value)
    }

    async fn tool_sql_readonly(&self, sql: String) -> Result<String, String> {
        let db = self.state.db.clone();
        let response = tokio::task::spawn_blocking(move || sql_query_blocking(&db, sql.as_str()))
            .await
            .map_err(|error| format!("sql worker join error: {error}"))?
            .map_err(|error| format!("sql query failed: {error}"))?;
        to_pretty_json(&response)
    }

    async fn tool_query_pack(&self, name: String, limit: Option<u32>) -> Result<String, String> {
        let limit = limit.unwrap_or(50);
        if limit == 0 {
            return Err("limit must be greater than 0".to_string());
        }
        let db = self.state.db.clone();
        let response = tokio::task::spawn_blocking(move || query_named_blocking(&db, &name, limit))
            .await
            .map_err(|error| format!("query worker join error: {error}"))?
            .map_err(|error| format!("query pack failed: {error}"))?;
        to_pretty_json(&response)
    }

    async fn tool_source_preview(&self, frame_id_raw: u64) -> Result<String, String> {
        let (frame_id, module_identity, rel_pc) = lookup_frame_source_by_raw(frame_id_raw)
            .ok_or_else(|| format!("unknown frame_id {frame_id_raw}"))?;
        let db = self.state.db.clone();
        let preview = tokio::task::spawn_blocking(move || {
            lookup_source_in_db(&db, frame_id, module_identity, rel_pc)
        })
        .await
        .map_err(|error| format!("source preview worker join error: {error}"))?
        .map_err(|error| format!("source preview lookup failed: {error}"))?;
        let preview = preview.ok_or_else(|| "source not available for frame".to_string())?;
        to_pretty_json(&preview)
    }

    async fn tool_source_previews(&self, frame_ids_raw: Vec<u64>) -> Result<String, String> {
        if frame_ids_raw.is_empty() {
            return Err("frame_ids must be non-empty".to_string());
        }
        let mut lookups = Vec::with_capacity(frame_ids_raw.len());
        let mut unknown_frame_ids = Vec::new();
        for frame_id_raw in frame_ids_raw {
            match lookup_frame_source_by_raw(frame_id_raw) {
                Some((frame_id, module_identity, rel_pc)) => {
                    lookups.push((frame_id, module_identity, rel_pc));
                }
                None => unknown_frame_ids.push(frame_id_raw),
            }
        }
        if !unknown_frame_ids.is_empty() {
            let rendered = unknown_frame_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("unknown frame_id values in batch: [{rendered}]"));
        }

        let db = self.state.db.clone();
        let response = tokio::task::spawn_blocking(move || {
            let mut previews = Vec::with_capacity(lookups.len());
            let mut unavailable_frame_ids: Vec<FrameId> = Vec::new();
            for (frame_id, module_identity, rel_pc) in lookups {
                match lookup_source_in_db(&db, frame_id, module_identity, rel_pc)? {
                    Some(preview) => previews.push(preview),
                    None => unavailable_frame_ids.push(frame_id),
                }
            }
            Ok::<SourcePreviewBatchResponse, String>(SourcePreviewBatchResponse {
                previews,
                unavailable_frame_ids,
            })
        })
        .await
        .map_err(|error| format!("source previews worker join error: {error}"))?
        .map_err(|error| format!("source previews lookup failed: {error}"))?;
        to_pretty_json(&response)
    }
}

#[async_trait]
impl ServerHandler for MoireMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _params: Option<PaginatedRequestParams>,
        _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: MoireTools::tools(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        params: CallToolRequestParams,
        _runtime: Arc<dyn rust_mcp_sdk::McpServer>,
    ) -> Result<CallToolResult, CallToolError> {
        let tool_name = params.name.clone();
        let args = params.arguments.unwrap_or_default();
        let response = match self.dispatch_tool(tool_name.as_str(), &args).await {
            Ok(body) => body,
            Err(error) => format!("Error: {error}"),
        };
        Ok(CallToolResult::text_content(vec![response.into()]))
    }
}

pub async fn run_mcp_server(listener: TcpListener, state: AppState) -> Result<(), String> {
    let local_addr = listener
        .local_addr()
        .map_err(|error| format!("resolve mcp listener addr: {error}"))?;
    let handler = MoireMcpHandler::new(state);
    let app_state = Arc::new(McpAppState {
        session_store: Arc::new(InMemorySessionStore::new()),
        id_generator: Arc::new(UuidGenerator {}),
        stream_id_gen: Arc::new(FastIdGenerator::new(Some("s_"))),
        server_details: Arc::new(server_details()),
        handler: handler.to_mcp_server_handler(),
        ping_interval: DEFAULT_MCP_PING_INTERVAL,
        transport_options: Arc::new(TransportOptions::default()),
        enable_json_response: false,
        event_store: None,
        task_store: None,
        client_task_store: None,
    });

    let http_handler = Arc::new(McpHttpHandler::new(vec![]));

    let app = Router::new()
        .route(
            DEFAULT_MCP_ENDPOINT,
            get(handle_streamable_http_get)
                .post(handle_streamable_http_post)
                .delete(handle_streamable_http_delete),
        )
        .with_state(app_state)
        .layer(Extension(http_handler));

    info!(
        endpoint = %DEFAULT_MCP_ENDPOINT,
        addr = %local_addr,
        "moire-web MCP Streamable HTTP ready"
    );

    axum::serve(listener, app)
        .await
        .map_err(|error| format!("MCP server failed: {error}"))
}

fn server_details() -> InitializeResult {
    InitializeResult {
        server_info: Implementation {
            name: "moire-web".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some("Moire runtime graph server with MCP tools".into()),
            title: Some("moire-web MCP".into()),
            icons: vec![],
            website_url: Some("https://github.com/bearcove/moire".into()),
        },
        capabilities: ServerCapabilities {
            tools: Some(ServerCapabilitiesTools { list_changed: None }),
            ..Default::default()
        },
        protocol_version: LATEST_PROTOCOL_VERSION.into(),
        instructions: Some(
            "Use moire MCP tools to inspect live connections, trigger cuts, and capture snapshots."
                .into(),
        ),
        meta: None,
    }
}

fn to_pretty_json<T>(value: &T) -> Result<String, String>
where
    for<'a> T: facet::Facet<'a>,
{
    facet_json::to_string_pretty(value).map_err(|error| format!("encode json response: {error}"))
}

fn required_non_empty_string(
    args: &JsonMap<String, JsonValue>,
    field: &str,
) -> Result<String, String> {
    let value = args
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("missing required `{field}` string argument"))?
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(format!("`{field}` must not be empty"));
    }
    Ok(value)
}

fn optional_u32(args: &JsonMap<String, JsonValue>, field: &str) -> Result<Option<u32>, String> {
    let Some(raw) = args.get(field) else {
        return Ok(None);
    };
    let raw = raw
        .as_u64()
        .ok_or_else(|| format!("`{field}` must be an unsigned integer"))?;
    u32::try_from(raw)
        .map(Some)
        .map_err(|_| format!("`{field}` value {raw} exceeds u32::MAX"))
}

fn required_u64(args: &JsonMap<String, JsonValue>, field: &str) -> Result<u64, String> {
    let value = args
        .get(field)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| format!("missing required `{field}` unsigned integer argument"))?;
    Ok(value)
}

fn required_u64_list(args: &JsonMap<String, JsonValue>, field: &str) -> Result<Vec<u64>, String> {
    let values = args
        .get(field)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| format!("missing required `{field}` array argument"))?;
    let mut out = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let numeric = value
            .as_u64()
            .ok_or_else(|| format!("`{field}[{index}]` must be an unsigned integer"))?;
        out.push(numeric);
    }
    Ok(out)
}

async fn handle_streamable_http_get(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
) -> Result<Response, TransportServerError> {
    let request = McpHttpHandler::create_request(Method::GET, uri, headers, None);
    let generic_response = http_handler.handle_streamable_http(request, state).await?;
    Ok(convert_response(generic_response))
}

async fn handle_streamable_http_post(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
    payload: String,
) -> Result<Response, TransportServerError> {
    let request =
        McpHttpHandler::create_request(Method::POST, uri, headers, Some(payload.as_str()));
    let generic_response = http_handler.handle_streamable_http(request, state).await?;
    Ok(convert_response(generic_response))
}

async fn handle_streamable_http_delete(
    headers: HeaderMap,
    uri: Uri,
    State(state): State<Arc<McpAppState>>,
    Extension(http_handler): Extension<Arc<McpHttpHandler>>,
) -> Result<Response, TransportServerError> {
    let request = McpHttpHandler::create_request(Method::DELETE, uri, headers, None);
    let generic_response = http_handler.handle_streamable_http(request, state).await?;
    Ok(convert_response(generic_response))
}

fn convert_response(response: axum::http::Response<GenericBody>) -> Response {
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, axum::body::Body::new(body))
}
