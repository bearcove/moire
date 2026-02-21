use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use moire_types::{QueryRequest, SqlRequest};

use crate::app::AppState;
use crate::db::{Db, query_named_blocking, sql_query_blocking};
use crate::util::http::{json_error, json_ok};

pub async fn api_sql(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    execute_sql_request(body, state.db.clone()).await
}

pub async fn api_query(State(state): State<AppState>, body: Bytes) -> impl IntoResponse {
    execute_named_query_request(body, state.db.clone()).await
}

pub async fn execute_sql_request(body: Bytes, db: Arc<Db>) -> impl IntoResponse {
    let request: SqlRequest = match facet_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid request json: {error}"),
            );
        }
    };

    match tokio::task::spawn_blocking(move || sql_query_blocking(&db, request.sql.as_str())).await {
        Ok(Ok(response)) => json_ok(&response),
        Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, error),
        Err(error) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("sql worker join error: {error}"),
        ),
    }
}

pub async fn execute_named_query_request(body: Bytes, db: Arc<Db>) -> impl IntoResponse {
    let request: QueryRequest = match facet_json::from_slice(&body) {
        Ok(request) => request,
        Err(error) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("invalid request json: {error}"),
            );
        }
    };

    let name = request.name.to_string();
    let limit = request.limit.unwrap_or(50);
    match tokio::task::spawn_blocking(move || query_named_blocking(&db, &name, limit)).await {
        Ok(Ok(response)) => json_ok(&response),
        Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, error),
        Err(error) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("query worker join error: {error}"),
        ),
    }
}
