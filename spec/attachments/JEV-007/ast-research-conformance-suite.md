# AST Research — JEV-007 Protocol Conformance Test Suite

Research performed during JEV-007 discovery (specifying) to pin down exactly
which seams the conformance suite drives, so the tests assert the real
protocol surface without duplicating JEV-003/004/005 unit coverage.

## 1. Public seams under test

### 1.1 Router factory (weight-free tier)
`src/server/router.rs`:
- `pub const MAX_BODY_BYTES: usize = 1_048_576` (1 MiB body cap)
- `pub fn build_router(state: ServerState) -> Router` — routes
  `POST /v1/classifier` + `POST /v1/systemone` (one shared handler,
  `super::handlers::classifier::classify`), `GET /health`, `GET /openapi.json`;
  layers: `TraceLayer::new_for_http()` then `DefaultBodyLimit::max(MAX_BODY_BYTES)`.
  Testable with `tower::ServiceExt::oneshot` (dev-dep `tower` already present,
  see `tests/router_endpoints.rs`), no sockets.

### 1.2 State / execution seam (weight-free tier)
`src/server/state.rs`:
- `pub struct ServerConfig { pub max_model_len: Option<usize>, pub max_request_branches: usize, pub max_queued: usize }`
  with `Default` = `{ None, 100, 16 }`. The suite exercises non-default values
  (`max_queued: 1` for the 429 scenario, `max_request_branches: 2` for the
  branch-limit scenario, `max_model_len` is not needed because the mock's
  `admit` reports lengths itself).
- `pub trait Answerer: Send + Sync` — `checkpoint_limits() -> (usize, usize)`,
  `admit(state, questions, effective_cap) -> Result<Vec<usize>, AdmissionError>`,
  `system_one(state, questions) -> anyhow::Result<Vec<(String, Answer)>>`.
  The mock implements this trait (pattern from `tests/router_endpoints.rs`
  `MockAnswerer` + `tests/serve_cli.rs` `MockAnswerer` with `forward_delay`).
  For the 429 scenario the mock must **block** in `system_one` (e.g. park on a
  shared `Condvar`/`Barrier` or sleep) so a second concurrent request hits the
  full admission queue; the mock must also record call counts (to assert no
  forward on 422 paths) and support one-shot failure (to drive the 500 path).
- `pub struct ServerState` — `Clone` over `Arc<Inner>`; `new(answerer: Arc<dyn Answerer>, model_name, config)`;
  `admission: Semaphore(max_queued + 1)` (1 in-flight + N waiting);
  `model_lock: tokio::sync::Mutex<()>`; `execute(state, questions)` is the
  end-to-end seam: admission 422 → queue 429 → serial forward → `ClassifierResponse`.
- `pub enum ExecutionError { Admission(AdmissionError), QueueFull, Internal(InternalError) }`
  — router maps these to 422/429/500 (see 1.4).

### 1.3 Lifecycle (model-gated tier)
`src/server/server.rs`:
- `pub async fn start_server(host, port, answerer, model_name, config) -> anyhow::Result<ServerHandle>` —
  binds (port 0 = ephemeral), runs `axum::serve` with graceful shutdown.
- `pub struct ServerHandle { pub port: u16, pub model_name: String, ... }` with `async fn stop(self)`.
- Model-gated tests load a real `RLAgent` (`rlcd::RLAgent::load(dir)`), wrap it
  in `Arc`, build `rlcd::server::RealAnswerer::new(agent)` (`src/server/state.rs:220`,
  `pub struct RealAnswerer { agent: Arc<RLAgent> }`), then `start_server("127.0.0.1", 0, ...)`.
  HTTP client: bare `TcpStream` round-trip (the `http()` helper in
  `tests/serve_cli.rs:126`) — dev-deps carry no HTTP client crate.

### 1.4 Handler pipeline (asserted via router responses)
`src/server/handlers/classifier.rs` `pub async fn classify`:
1. over-1 MiB body → 422 envelope (param `body`, "request body exceeds the 1 MiB limit")
2. non-`application/json` Content-Type → 422 (param `Content-Type`)
3. `parse_classifier_request` (JEV-003) → 422 envelope
4. `request.model != state.model_name()` → 422 `Loaded model is '<name>'`
5. `state.execute(...)`:
   - `ExecutionError::Admission(e)` → 422 (param `questions`, message `e.message()`)
   - `ExecutionError::QueueFull` → 429 `{"detail":"Scoring queue is full"}` + `Retry-After: 1`
   - `ExecutionError::Internal(_)` → 500 `{"detail":"internal error"}`
   - `Ok(response)` → 200 `response.to_json()`

### 1.5 Request validation (JEV-003 — pinned at HTTP level by this suite)
`src/server/request.rs` `pub fn parse_classifier_request(bytes) -> Result<ClassifierRequest, Vec<RequestError>>`:
- `RequestError { param, message, type_ }`; errors accumulate (envelope folds up to 10 details).
- Context XOR: `state` (string/object/array, null = absent) vs `messages`
  (nonempty array); violation → `err("state/messages", "context_conflict", "Provide exactly one of state or messages")`.
- Unknown top-level fields ignored; unknown question fields → 422 with dotted
  param `questions.<qid>.<field>`; unknown options fields → `options.<field>`;
  `options.raw_logits: true` → "raw_logits diagnostics are not supported by the laya backend".
- `type` outside choice/score/noul → 422; choice criteria 2–50; score 2–50;
  noul criteria only `true`/`false` keys.
- `questions` object 1–256 entries, nonempty keys.
- Reserved fields `tools`/`mm_processor_kwargs`/`media_io_kwargs`: nonempty → 422,
  empty/absent → accepted.
- Text-only messages: role ∈ system/developer/user/assistant, string content;
  image parts / tool role / extra fields → 422 (params `messages[i].<field>`).

### 1.6 Error envelopes (JEV-004 — byte shapes asserted by this suite)
`src/server/handlers/error.rs`:
- `pub const MAX_DETAILS: usize = 10`
- `validation_422(errors)` → `{"error":{"message","type":"invalid_request_error","code":422,"param","details":[{param,message,type}...]}}`
- `queue_full_429()` → 429, header `retry-after: 1`, body `{"detail":"Scoring queue is full"}`
- `client_disconnected_499()` → `{"detail":"Client disconnected"}`
- `internal_500()` → `{"detail":"internal error"}`
- `health.rs` → 200 `{"status":"ready","model":"<name>"}` (no inference;
  `mock.forwards() == 0` assertable).
- `openapi.rs` → 200 static OpenAPI 3.1 doc with paths
  `/v1/classifier`, `/v1/systemone`, `/health` (see `tests/router_endpoints.rs::the_openapi_document_is_served_for_discovery`).

### 1.7 Response mapping (JEV-005 — protocol shapes asserted by this suite)
`src/server/response.rs` `answer_to_jev_json`:
- choice: `{"type":"choice","choice","confidence","probabilities"}` (candidate
  keys in insertion order; **no `act_probability`**).
- score: `{"type":"score","score","confidence","probabilities" ("0".."N-1"), "legend"}`.
- noul: `{"type":"noul","noul"}` (**no confidence field**).
- `ClassifierResponse::to_json` → `{"model","answers" (request order),"usage":{"input_tokens","output_tokens"}}`;
  `usage.input_tokens` = sum of `admit` lengths; `output_tokens` always 0.

### 1.8 Agent numerics (model-gated numerical sanity)
`src/agent.rs` `RLAgent::system_one`: softmax over marker logits
(temperature-calibrated); choice `confidence = confidence_from_probs(p)` (max),
score = expected index `sum(p[i]*i)`, noul = `p[1]` (P(true)).
`RLAgent::checkpoint_limits() -> (max_len, head_max_len)`; admission overflow
(`admit` → `ExceedsTokenCap`) happens **before** the forward — a model-gated
test asserts 422 + zero forwards for an over-long state (a real
`RLAgent::checkpoint_limits()` cap is used; the over-long state is a string of
`"x " * (cap * 2)`-ish length — well beyond any cap, no truncation occurs at
the protocol layer because `admit` measures the *natural* length with
`NATURAL_CAP` and rejects before `system_one`).

## 2. Existing test conventions (mirrored by this suite)

- `tests/router_endpoints.rs` — weight-free oneshot style: `req(app, method, uri, body, content_type)` +
  `take(res) -> (StatusCode, String)` + `assert_422_envelope(status, body, detail_hint)`;
  mock `Answerer` with `AtomicUsize` call counter and `AtomicBool` one-shot failure.
- `tests/serve_cli.rs` — model-gated style: `model_dir() -> Option<String>` from
  `LAYA_TEST_MODEL`; `None => return;` (skip, not fail); bare-`TcpStream`
  `http(host_port, method, path, body) -> Option<(u16, String)>`; `free_port()`;
  `start_server(...).await` with `handle.stop().await`.
- `tests/cli_ask.rs` / `tests/agent.rs` — the `LAYA_TEST_MODEL` gate:
  `std::env::var("LAYA_TEST_MODEL").ok()` → `None => return` with the
  "skipped: no checkpoint available" comment.
- Fixtures: card doc says keep them small; the mixed fixture reuses the README
  refund example ("We were billed twice for March. Please refund the duplicate."
  + route/urgency/refund questions, README.md:109-121).

## 3. Concurrency for the 429 scenario (design decision)

`ServerState::execute` admits with `try_acquire_owned()` (non-blocking) on a
`Semaphore(max_queued + 1)`. With `max_queued: 1` the semaphore has 2 permits:
one for the in-flight forward + one waiting slot. To fill it deterministically
without timers:
- mock `Answerer::system_one` parks until released (a `std::sync::Arc<std::sync::Condvar + Mutex<bool>>`
  gate, or `tokio::task::block_in_place` + sleep). Since `system_one` runs in
  `spawn_blocking`, a blocking park is fine.
- request A (holds the in-flight permit while its forward blocks) + request B
  (takes the waiting permit, blocks on the model lock) → semaphore exhausted →
  request C → 429. Then release the gate; A and B complete; the server stays up.
- Assert: 429 body + `retry-after: 1` header; `mock.forwards()` count after
  release == 2 (A, B) — the 429 request never forwarded.

## 4. Model-gated overflow-without-inference design

The `RealAnswerer` wraps the real `RLAgent`; forwards are not countable.
Instead: the over-long state makes `admit` fail (`ExceedsTokenCap`) **before**
`system_one` is ever called — so the suite asserts the 422 envelope names the
question id and the cap. "No inference" is guaranteed by construction (the
`execute` pipeline order, pinned by the weight-free tier with a counting mock);
the model-gated tier only pins that the *real* `RLAgent` tokenizer agrees with
the admission arithmetic on a genuinely over-long state.

## 5. Files created/changed by this card (planned)

- NEW `tests/jev_conformance.rs` (the suite; weight-free + model-gated tiers)
- NEW `tests/jev_conformance/mixed_fixture.json` (README refund example, mixed choice/score/noul)
- NEW `tests/jev_conformance/chat_history.json` (3-turn text chat: system/user/assistant)
- NEW `tests/jev_conformance/wrong_model.json` (mixed body with `model: "typesafe-ai/jev"`)
- (weight-free bodies for the validation matrix are built inline — small
  single-purpose JSON strings, matching the `tests/request_schema.rs` style;
  only the three named protocol-shaped fixtures go to files, per the card doc's
  "keep them small" note)
