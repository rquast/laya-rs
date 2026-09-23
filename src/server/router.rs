//! The Jev-protocol router (JEV-004): one axum router exposing the full
//! protocol surface — `POST /v1/classifier`, its exact alias
//! `POST /v1/systemone` (same handler, identical behavior), `GET /health`,
//! and `GET /openapi.json`.
//!
//! Mirrors a standard axum router-factory pattern: a `build_router(state)`
//! free function (test-injectable, no sockets), a `Clone`-over-`Arc` state
//! newtype injected via the axum `State` extractor, a route factory, and a
//! `TraceLayer::new_for_http()` observability layer.
//!
//! `DefaultBodyLimit::max(1 MiB)` bounds request bodies protocol-wide; the
//! classifier handler maps the resulting `BytesRejection` to the 422
//! envelope (docs/jev-server/JEV-004-http-endpoints.md pipeline step 1).
//!
//! The `start_server`/`ServerHandle` lifecycle (graceful shutdown, the
//! `laya serve` CLI wiring) is JEV-006; this module is the pure router
//! factory so the endpoint tests can drive it via `Router::oneshot`.

use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::server::state::ServerState;

/// The protocol's request-body cap (the reference server's 1 MiB).
pub const MAX_BODY_BYTES: usize = 1_048_576;

/// Build the Jev-protocol router from an already-constructed
/// [`ServerState`].
pub fn build_router(state: ServerState) -> Router {
    Router::new()
        .route("/v1/classifier", post(super::handlers::classifier::classify))
        .route("/v1/systemone", post(super::handlers::classifier::classify)) // exact alias
        .route("/health", get(super::handlers::health::health))
        .route("/openapi.json", get(super::handlers::openapi::openapi))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}
