use std::io::Read;
use std::str::FromStr;
use std::time::Duration;

use axum::body::{self, Body};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;

use crate::util::http::{copy_request_headers, skip_request_header, skip_response_header};

struct ProxiedResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

pub async fn proxy_vite_request(
    base_url: &str,
    request: Request,
    body_limit_bytes: usize,
) -> axum::response::Response {
    let (parts, body) = request.into_parts();
    let method = parts.method.as_str().to_string();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let url = format!("{base_url}{path_and_query}");
    let headers = copy_request_headers(&parts.headers);
    let body = match body::to_bytes(body, body_limit_bytes).await {
        Ok(body) => body.to_vec(),
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read request body: {error}"),
            )
                .into_response();
        }
    };

    let proxied = match tokio::task::spawn_blocking(move || {
        proxy_vite_blocking(&method, &url, headers, body)
    })
    .await
    {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return (StatusCode::BAD_GATEWAY, error).into_response(),
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("proxy worker join error: {error}"),
            )
                .into_response();
        }
    };

    build_proxy_response(proxied)
}

fn proxy_vite_blocking(
    method: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
) -> Result<ProxiedResponse, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(2))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut req = agent.request(method, url);

    for (name, value) in headers {
        if skip_request_header(&name) {
            continue;
        }
        req = req.set(&name, &value);
    }

    let resp = if body.is_empty() && (method == "GET" || method == "HEAD") {
        match req.call() {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(error)) => {
                return Err(format!("Vite proxy request failed for {url}: {error}"));
            }
        }
    } else {
        match req.send_bytes(&body) {
            Ok(resp) => resp,
            Err(ureq::Error::Status(_, resp)) => resp,
            Err(ureq::Error::Transport(error)) => {
                return Err(format!("Vite proxy request failed for {url}: {error}"));
            }
        }
    };

    let status = resp.status();
    let mut response_headers = Vec::new();
    for name in resp.headers_names() {
        for value in resp.all(&name) {
            response_headers.push((name.clone(), value.to_string()));
        }
    }

    let mut response_body = Vec::new();
    resp.into_reader()
        .read_to_end(&mut response_body)
        .map_err(|error| format!("failed reading Vite proxy response body: {error}"))?;

    Ok(ProxiedResponse {
        status,
        headers: response_headers,
        body: response_body,
    })
}

fn build_proxy_response(proxied: ProxiedResponse) -> axum::response::Response {
    let status = StatusCode::from_u16(proxied.status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = axum::response::Response::new(Body::from(proxied.body));
    *response.status_mut() = status;

    for (name, value) in proxied.headers {
        if skip_response_header(&name) {
            continue;
        }
        let Ok(header_name) = header::HeaderName::from_str(&name) else {
            continue;
        };
        let Ok(header_value) = header::HeaderValue::from_str(&value) else {
            continue;
        };
        response.headers_mut().append(header_name, header_value);
    }

    response
}
