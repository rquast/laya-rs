//! The classifier handler — the single handler behind both
//! `POST /v1/classifier` and its exact alias `POST /v1/systemone` (JEV-004).
//!
//! Pipeline (docs/jev-server/JEV-004-http-endpoints.md):
//! 1. Body guard: `DefaultBodyLimit::max(1 MiB)` on the router — an
//!    over-limit body rejects the `Bytes` extractor, mapped to the 422
//!    envelope (no JSON parsing happens).
//! 2. Content-Type guard: missing or `application/json` (any parameters) is
//!    tolerated; anything else is a 422 before parsing.
//! 3. JEV-003 strict parse + validate → 422 envelope.
//! 4. Model identity: `request.model == state.model_name()`, else 422
//!    `Loaded model is '<name>'`.
//! 5. JEV-005 execution seam: admission queue (429), serial forward (500 on
//!    internal error), protocol response (200).
//!
//! The handler never panics on malformed input: every failure path yields a
//! readable protocol status.

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::State;
use axum::http::header;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::server::request::{canonical, parse_classifier_request, ClassifierRequest, Context, RequestError};
use crate::server::state::{ExecutionError, ServerState};

use super::error;

/// One classifier request (shared by `/v1/classifier` and `/v1/systemone`).
pub async fn classify(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    // 1. Body guard: over-limit bodies never reach JSON parsing.
    let bytes = match body {
        Ok(b) => b,
        Err(_) => {
            return error::validation_422(vec![RequestError {
                param: "body".to_string(),
                message: "request body exceeds the 1 MiB limit".to_string(),
                type_: "body_limit".to_string(),
            }])
        }
    };

    // 2. Content-Type guard: JSON only (missing tolerated — clients probing
    //    without a content type still get a readable 422).
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        let ct = ct.to_str().unwrap_or_default();
        let mime = ct.split(';').next().unwrap_or_default().trim();
        if mime != "application/json" && !mime.ends_with("+json") {
            return error::validation_422(vec![RequestError {
                param: "Content-Type".to_string(),
                message: format!("Content-Type must be application/json (got '{mime}')"),
                type_: "unsupported_content_type".to_string(),
            }])
        }
    }

    // 3. Strict parse + validate (JEV-003).
    let request = match parse_classifier_request(&bytes) {
        Ok(r) => r,
        Err(errors) => return error::validation_422(errors),
    };

    // 4. Model identity (the laya backend is loaded with exactly one model).
    if request.model != state.model_name() {
        return error::validation_422(vec![RequestError {
            param: "model".to_string(),
            message: format!("Loaded model is '{}'", state.model_name()),
            type_: "unknown_model".to_string(),
        }])
    }

    // 5. Execute (JEV-005): admission queue → serial forward → response.
    let state_value = context_to_value(&request);
    match state.execute(state_value, request.questions).await {
        Ok(response) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            );
            (StatusCode::OK, headers, response.to_json()).into_response()
        }
        Err(ExecutionError::QueueFull) => error::queue_full_429(),
        Err(ExecutionError::Admission(e)) => error::validation_422(vec![RequestError {
            param: "questions".to_string(),
            message: e.message(),
            type_: "admission".to_string(),
        }]),
        Err(ExecutionError::Internal(_)) => error::internal_500(),
    }
}

/// The request context as the `Value` the execution seam consumes: `state`
/// passes through; `messages` becomes a JSON list of `{role, content}`
/// objects (the reference's serialization for the laya backend —
/// architecture.md pipeline step 5).
fn context_to_value(request: &ClassifierRequest) -> serde_json::Value {
    match &request.context {
        Context::State(state) => state.clone(),
        Context::Messages(messages) => {
            let items = messages
                .iter()
                .map(|m| {
                    let role = match m.role {
                        crate::server::request::Role::System => "system",
                        crate::server::request::Role::Developer => "developer",
                        crate::server::request::Role::User => "user",
                        crate::server::request::Role::Assistant => "assistant",
                    };
                    format!(
                        "{{\"role\":{},\"content\":{}}}",
                        canonical(&serde_json::Value::String(role.to_string())),
                        canonical(&serde_json::Value::String(m.content.clone()))
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            // The parse already validated the entries (string content), so
            // this reconstruction cannot fail.
            serde_json::from_str(&format!("[{items}]")).expect("validated messages serialize")
        }
    }
}
