//! `GET /health` (JEV-004): readiness without inference.
//!
//! `{"status":"ready","model":"<loaded model>"}` — 200. The server only
//! listens after the model is loaded (JEV-006), so there is no "loading"
//! state to report; the endpoint is pure state read, never a forward pass.

use axum::extract::State;
use axum::http::header;
use axum::http::StatusCode;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::response::Response;

use crate::server::state::ServerState;

pub async fn health(State(state): State<ServerState>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    let body = format!(
        "{{\"status\":\"ready\",\"model\":{}}}",
        serde_json::to_string(state.model_name()).expect("string serializes")
    );
    (StatusCode::OK, headers, body).into_response()
}
