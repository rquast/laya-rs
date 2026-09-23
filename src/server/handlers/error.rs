//! Protocol error envelopes (JEV-004) — the one shared place for every
//! failure shape of the classifier endpoint.
//!
//! - 422: `{"error":{message,type:"invalid_request_error",code:422,param,
//!   details[]}}` with at most [`MAX_DETAILS`] detail entries; extra
//!   validation errors are folded into the summary `message`.
//! - 429: short form `{"detail":"Scoring queue is full"}` + `Retry-After: 1`.
//! - 499: short form `{"detail":"Client disconnected"}` (deliverable only if
//!   the client can still receive a response — axum drops the task when the
//!   connection goes away, so this builder is the seam-level representation;
//!   JEV-006/007 use it for the lifecycle layer).
//! - 500: short form `{"detail":"internal error"}` for unhandled runtime
//!   failures (no stable structured body guaranteed).
//!
//! Every builder sets `Content-Type: application/json`; the classifier
//! handler maps [`ExecutionError`](crate::server::state::ExecutionError) and
//! the JEV-003 `RequestError`s to these — see [`super::classifier`].

use axum::http::header;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::server::request::RequestError;

/// The protocol's detail cap: at most this many `details` entries ride the
/// 422 envelope; the rest are counted in the summary message.
pub const MAX_DETAILS: usize = 10;

/// Build a JSON response: `Content-Type: application/json`, the given
/// status, optional extra headers, and the body.
fn json_response(status: StatusCode, extra: Option<(&'static str, &'static str)>, body: impl Into<String>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
    if let Some((name, value)) = extra {
        headers.insert(
            axum::http::HeaderName::from_static(name),
            header::HeaderValue::from_static(value),
        );
    }
    (status, headers, body.into()).into_response()
}

/// A detail entry of the 422 envelope (document key order:
/// `param`, `message`, `type`).
fn detail_json(e: &RequestError) -> String {
    let s = |v: &str| serde_json::to_string(v).expect("string serializes");
    format!("{{\"param\":{},\"message\":{},\"type\":{}}}", s(&e.param), s(&e.message), s(&e.type_))
}

/// The 422 envelope from a list of JEV-003 validation errors (or one
/// synthesized error for body-limit / model-identity / admission failures).
///
/// Summary semantics: a single error names itself as the message; multiple
/// errors name the first plus a count; more than [`MAX_DETAILS`] errors
/// show the first 10 details and fold the remainder into the message.
pub fn validation_422(errors: Vec<RequestError>) -> Response {
    assert!(!errors.is_empty(), "the 422 envelope requires at least one error");
    let param = errors[0].param.clone();
    let message = if errors.len() > MAX_DETAILS {
        format!(
            "{} validation errors ({} details shown, {} more): {}",
            errors.len(),
            MAX_DETAILS,
            errors.len() - MAX_DETAILS,
            errors[0].message
        )
    } else if errors.len() > 1 {
        format!("{} validation errors: {} (+{} more)", errors.len(), errors[0].message, errors.len() - 1)
    } else {
        errors[0].message.clone()
    };
    let details = errors
        .iter()
        .take(MAX_DETAILS)
        .map(detail_json)
        .collect::<Vec<_>>()
        .join(",");
    let s = |v: &str| serde_json::to_string(v).expect("string serializes");
    let body = format!(
        "{{\"error\":{{\"message\":{},\"type\":\"invalid_request_error\",\"code\":422,\"param\":{},\"details\":[{}]}}}}",
        s(&message),
        s(&param),
        details
    );
    json_response(StatusCode::UNPROCESSABLE_ENTITY, None, body)
}

/// The 429 short form: the admission queue (1 in-flight + `max_queued`
/// waiting) is full.
pub fn queue_full_429() -> Response {
    json_response(
        StatusCode::TOO_MANY_REQUESTS,
        Some(("retry-after", "1")),
        "{\"detail\":\"Scoring queue is full\"}",
    )
}

/// The 499 short form (best effort — only if the response can still reach
/// the client).
pub fn client_disconnected_499() -> Response {
    json_response(
        StatusCode::from_u16(499).expect("499 is a valid status"),
        None,
        "{\"detail\":\"Client disconnected\"}",
    )
}

/// The 500 short form for unhandled runtime failures (e.g. a model forward
/// error). No stable structured body is guaranteed by the protocol.
pub fn internal_500() -> Response {
    json_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        None,
        "{\"detail\":\"internal error\"}",
    )
}
