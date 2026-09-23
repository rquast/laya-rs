# AST Research: JEV-004 HTTP Endpoints

Research performed with AstGrep + targeted reads before specifying the
`src/server/router.rs` + `src/server/handlers/` implementation.

## 1. Reference pattern: axum router factory (mirrored by JEV-004)

The target pattern (documented in `docs/jev-server/architecture.md`,
"Router factory" row): a `build_router(state) -> Router` free function that
is test-injectable (no sockets), a `Clone` state newtype over `Arc<Inner>`
injected via the axum `State` extractor, a route factory, and
`TraceLayer::new_for_http()` layers. Lifecycle (bind + graceful shutdown)
is a separate concern (JEV-006) following the same handle pattern:
`start_* -> Handle { port, shutdown: oneshot::Sender, task: JoinHandle }`
with `async fn stop(self)` that sends the oneshot and awaits the task.

```rust
pub fn build_router(state: ServerState) -> Router {
    Router::new()
        .route("/v1/classifier", post(handlers::classifier::classify))
        .route("/v1/systemone", post(handlers::classifier::classify))
        .route("/health", get(handlers::health::health))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}
```

- **State**: `#[derive(Clone)] pub struct ServerState { inner: Arc<Inner> }`
  (JEV-005) — directly usable as the axum `State` extractor.
- **Handlers**: `pub async fn health(State(state): State<ServerState>) -> Response`;
  the classifier handler takes `Result<Bytes, BytesRejection>` so the
  body-limit rejection is mappable to a protocol 422.
- **Lifecycle**: `TcpListener::bind(host:port)` ->
  `axum::serve(listener, app).with_graceful_shutdown(rx.await)` -> handle
  struct; `stop()` sends the oneshot, awaits the task.

JEV-004 adopts: `build_router(state)`, the Clone-over-Arc state newtype,
`TraceLayer::new_for_http()`, and the handler module layout.
Deviations (documented in the feature file): the router adds
`DefaultBodyLimit::max(1_048_576)` plus one POST route pair; the
`start_server`/`ServerHandle` lifecycle is JEV-006 (separate card);
JEV-004 delivers `build_router` + handlers + error envelope only.

## 2. Existing laya-rs server seams (JEV-003/JEV-005 deliverables)

`src/server/mod.rs` exports:
- `request::{parse_classifier_request, ClassifierRequest, Context, RequestError, ...}`
  (JEV-003). `parse_classifier_request(bytes: &[u8]) ->
  Result<ClassifierRequest, Vec<RequestError>>`; `ClassifierRequest { model:
  String, context: Context, questions: Vec<(String, Question)> }`;
  `Context::State(Value) | Context::Messages(Vec<ChatMessage>)`;
  `RequestError { param, message, type_ }` (dotted paths, `[i]` indices).
- `response::{answer_to_jev_json, ClassifierResponse, Usage}` (JEV-005).
  `ClassifierResponse::to_json() -> String` (document key order).
- `state::{ServerState, ServerConfig, Answerer, ExecutionError, ...}` (JEV-005).
  `ServerState::execute(state: Value, questions) ->
  Result<ClassifierResponse, ExecutionError>`;
  `ExecutionError::{Admission(AdmissionError), QueueFull, Internal(InternalError)}`;
  `ServerState::model_name() -> &str`.

Integration decisions derived from this AST research:
- `Context::Messages` -> `serde_json::Value` list of `{role, content}` objects
  before `execute` (architecture.md pipeline step 5; reference behavior).
- Model-identity check happens in the handler between JEV-003 parse and
  JEV-005 execute: `request.model != state.model_name()` -> 422
  `Loaded model is '<name>'` (protocol-reference.md admission rule).
- `ExecutionError` -> status mapping: `Admission` -> 422 envelope,
  `QueueFull` -> 429 short form + `Retry-After: 1`, `Internal` -> 500
  short form.

## 3. Dependency gating (Cargo.toml)

- `tokio` (rt, sync) already lives under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`; dev-deps add
  `macros`, `time`.
- axum 0.8 + tower-http (trace-only) join that same target-gated block, so
  the `wasm32-unknown-unknown` build (guarded by `tests/wasm_compile.rs`) is
  untouched.
