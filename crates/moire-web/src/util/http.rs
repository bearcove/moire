use axum::http::{HeaderMap, StatusCode, header};
use axum::response::IntoResponse;
use facet::Facet;
use moire_types::ApiError;

pub fn copy_request_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

pub fn skip_request_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "host" || lower == "content-length" || is_hop_by_hop(&lower)
}

pub fn skip_response_header(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "content-length" || is_hop_by_hop(&lower)
}

pub fn json_ok<T>(value: &T) -> axum::response::Response
where
    T: for<'facet> Facet<'facet>,
{
    match facet_json::to_string(value) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("json encode error: {error}"),
        )
            .into_response(),
    }
}

pub fn json_error(status: StatusCode, message: impl Into<String>) -> axum::response::Response {
    json_with_status(
        status,
        &ApiError {
            error: message.into(),
        },
    )
}

pub fn json_with_status<T>(status: StatusCode, value: &T) -> axum::response::Response
where
    T: for<'facet> Facet<'facet>,
{
    match facet_json::to_string(value) {
        Ok(body) => (
            status,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!("json encode error: {error}"),
        )
            .into_response(),
    }
}

fn is_hop_by_hop(lowercase_name: &str) -> bool {
    matches!(
        lowercase_name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}
