use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;

use crate::server::DashboardState;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

pub fn router(state: Arc<DashboardState>) -> Router {
    Router::new()
        .route("/api/dumps", get(api_dumps))
        .fallback(static_handler)
        .with_state(state)
}

async fn api_dumps(State(state): State<Arc<DashboardState>>) -> Response {
    let dumps = state.all_dumps().await;
    match facet_json::to_string(&dumps) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialization error: {e}"),
        )
            .into_response(),
    }
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first
    if !path.is_empty() {
        if let Some(file) = FrontendAssets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data,
            )
                .into_response();
        }
    }

    // SPA fallback: serve index.html for unknown paths
    match FrontendAssets::get("index.html") {
        Some(file) => Html(file.data).into_response(),
        None => (StatusCode::NOT_FOUND, "frontend not built").into_response(),
    }
}
