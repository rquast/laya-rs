# AST Research — JEV-002 Architecture & Protocol Reference (parent card closure)

JEV-002 is the jev-server epic's parent card: its deliverable is the
architecture + protocol reference (docs/jev-server/architecture.md and
protocol-reference.md, both attached). This research records that the
architecture those docs describe has in fact been built, by verifying each
stated component exists in the code (AST-verified) and naming the existing
tests that prove each scenario — JEV-002's scenarios are closed by the
child cards' test suites, not by new code.

## 1. Stated components -> code (verified via AstGrep/Read)

### 1.1 "axum 0.8 Router + tokio runtime, tower-http TraceLayer"
- `src/server/router.rs:32` — `pub fn build_router(state: ServerState) -> Router`:
  routes `POST /v1/classifier` + `POST /v1/systemone` (one shared handler,
  `handlers::classifier::classify`), `GET /health`, `GET /openapi.json`;
  layers `TraceLayer::new_for_http()` + `DefaultBodyLimit::max(1 MiB)`.
- `Cargo.toml` — `axum = "0.8.9"`, `tower-http = "0.6.11" (default-features off,
  features = ["trace"])`, `tokio` — all under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, i.e. native-only
  (rule 3: the wasm32 build is untouched; `tests/wasm_compile.rs` guards it).

### 1.2 "build_router(state) factory, Clone-over-Arc state newtype via State extractor"
- `src/server/state.rs:266` — `pub struct ServerState { inner: Arc<Inner> }`
  with `#[derive(Clone)]`; `Inner { answerer: Arc<dyn Answerer>, model_name,
  config, model_lock: Arc<Mutex<()>>, admission: Arc<Semaphore> }`.
- `src/server/handlers/classifier.rs:34` — `classify(State(state): State<ServerState>, ...)`
  (the axum State extractor).

### 1.3 "start_server -> ServerHandle { port, oneshot shutdown, JoinHandle }, graceful shutdown"
- `src/server/server.rs:54` — `pub async fn start_server(host, port, answerer,
  model_name, config) -> anyhow::Result<ServerHandle>`; port 0 = ephemeral.
- `src/server/server.rs:29` — `pub struct ServerHandle { pub port: u16,
  pub model_name: String, shutdown: oneshot::Sender<()>, task: JoinHandle<()> }`
  with `stop()` (graceful: in-flight forwards complete — verified by
  tests/serve_cli.rs `ctrl_c_stops_the_server_gracefully`).

### 1.4 "Protocol layer never imports candle types; touches RLAgent only at system_one"
- `src/server/request.rs`, `src/server/response.rs` — pure over
  `serde_json::Value` / `rlcd::Answer` (no candle imports; request.rs is pure
  over `&[u8]`, response.rs serializes `Answer` to the protocol shape).
- `src/server/state.rs:54` — `pub trait Answerer: Send + Sync` (the seam);
  `RealAnswerer` (state.rs:220) wraps `Arc<RLAgent>` and is the only place
  the layer meets the model; `ServerState::execute` (state.rs:332) runs
  admission -> 429 queue -> serial forward (`spawn_blocking`) -> mapping.

### 1.5 "Open Simple-Jev v1 contract; TypeSafe hosted envelope out of scope"
- Endpoints are exactly `POST /v1/classifier` + alias `POST /v1/systemone`,
  `GET /health` (router.rs:32-38); error envelope shapes match
  protocol-reference.md (handlers/error.rs: `validation_422`,
  `queue_full_429` + `Retry-After: 1`, `internal_500`).

### 1.6 "Decomposed into children JEV-003 -> JEV-005 -> JEV-004 -> JEV-006 -> JEV-007"
- All five children are `done` (board). Each has its own feature file
  (spec/features/*) and card doc (docs/jev-server/JEV-00{3,4,5,6,7}-*.md).

## 2. Scenario -> existing proof (JEV-002's five scenarios, closed by child suites)

| JEV-002 scenario | Proven by (existing, passing) |
| --- | --- |
| "A Jev-protocol client completes the quickstart" (/health + POST /v1/classifier, output_tokens 0) | tests/serve_cli.rs (health round-trip on the real binary) + tests/jev_conformance.rs `model_gated_contract_round_trips_assert_the_protocol_shape` (real checkpoint: /health polling, mixed POST, usage.output_tokens == 0, determinism) |
| "The router exposes the protocol surface on a single axum router" (alias shares handler; health; openapi) | tests/router_endpoints.rs (`the_systemone_alias_behaves_identically_to_classifier`, `health_reports_readiness_without_inference`, `the_openapi_document_is_served_for_discovery`) |
| "The HTTP stack follows a proven axum serving pattern" (start_server/handle/graceful stop) | tests/serve_cli.rs (`serve_loads_the_checkpoint_and_serves_on_the_requested_port`, `ctrl_c_stops_the_server_gracefully`, `the_effective_sequence_cap_clamps_to_the_checkpoints_native_max_len`) |
| "The protocol layer stays weight-free testable" (mock answerer, no checkpoint/GPU) | tests/jev_conformance.rs weight-free tier (`the_weight_free_tier_passes_with_no_checkpoint_and_no_gpu`, `the_suite_pins_the_validation_matrix`) + tests/request_schema.rs (parse/validate without weights) |
| "The epic decomposes into ordered child stories" (each child: feature + card doc; conformance gate) | board state: JEV-003..007 all done; card docs docs/jev-server/JEV-00{3,4,5,6,7}-*.md exist; tests/jev_conformance.rs is the gate (JEV-007) |

## 3. Verification status (at closure)

- `cargo test` (all 19 test binaries) green, LAYA_TEST_MODEL unset
  (model-gated scenarios skip, not fail).
- `LAYA_TEST_MODEL=~/.cache/rlcd-rs/laya-typed-decisions cargo test --test
  jev_conformance` green (real-checkpoint round-trips, incl. numerical
  sanity after the confidence=max-probability fix in answer_to_jev_json).
- `cargo clippy --all-targets`: no new warnings from the jev-server code
  (remaining warnings pre-existing; the doc-comment `/**` style is the
  established convention across all 17 test files).
- `fspec check` / `validate` / `validate-tags` / `audit-coverage`: all pass.
