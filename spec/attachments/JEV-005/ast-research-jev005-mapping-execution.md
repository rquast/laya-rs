# AST Research — JEV-005 (laya-native answer mapping, usage, serial execution + 429 queue)

Research date: 2026-09-22. Method: direct source reads (src/schema.rs, src/agent.rs,
src/server/request.rs, src/lib.rs, Cargo.toml, tests/schema.rs, tests/agent.rs) +
GraphSearch `ast_search` over the AST index.

## Entities inspected

### `schema-rs::build_sequence` (src/schema.rs:89-159, complexity 11)
```
pub fn build_sequence(
    tok: &Tokenizer,
    special: &SpecialTokens,
    state: &Value,
    q: &Question,
    max_len: usize,
    head_max_len: usize,
) -> BuiltSequence { ids: Vec<u32>, markers: Vec<usize> }
```
- `ids` are truncated to `max_len` and `markers` are retained only while `< max_len`
  (lines 155-156). **Truncation is silent** — the caller detects marker loss via
  `built.markers.len() != render_options(q).len()` (the exact check `system_one`
  already performs, agent.rs:135-138).
- Sequence layout: `[CLS] head [SEP] [MASK]opt0... [MASK]optN... [SEP] state [SEP]`.
  State is re-tokenized per call → per-question length ≈ repeated context (the
  protocol's "repeated context is counted" usage rule, protocol-reference.md).
- Implication for JEV-005 admission: pre-build each sequence with
  `max_len = min(cfg max_len, --max-model-len)` and reject when
  `built.markers.len() != n_opts` or `built.ids.len()` would have been truncated
  (detect truncation by comparing `ids.len() == max_len` after construction, or by
  rebuilding the pre-truncate length: simpler — build with cap and check both
  marker integrity AND that the full (untruncated) length ≤ cap. The untruncated
  length is `built.ids.len()` when no truncation happened; truncation sets
  `ids.len() == max_len`, so the check is: markers intact AND ids.len() < max_len
  OR the exact pre-truncate size fits. Cleanest: build with a huge max_len to
  measure the natural length, then compare to the cap.)

### `agent-rs::system_one` (src/agent.rs:126-207, complexity 12)
```
pub fn system_one(&self, state: &Value, questions: &[(String, Question)])
    -> anyhow::Result<Vec<(String, Answer)>>
```
- Builds sequences internally with `self.cfg.max_len` / `self.cfg.head_max_len`
  (NOT caller-provided caps) — so a server that wants its own `--max-model-len`
  cap must pre-check separately (it does, JEV-005 admission) and note the real
  forward always uses the checkpoint's native `max_len`.
- `Answer` variants (agent.rs:55-59):
  - `Choice { choice: String, probabilities: Vec<(String, f32)>, confidence: f32, act_probability: f32 }`
    — probabilities are keyed zipped with `q.choice_criteria` keys **in request
    insertion order** (keys built at line 186, zip at line 190). This is the
    request-order guarantee the Jev mapping must preserve into JSON.
  - `Score { score: f32, legend: Vec<String>, probabilities: Vec<f32>, confidence: f32, act_probability: f32 }`
  - `Noul { noul: f32, act_probability: f32 }`
- `act_probability` is the rlcd action head — must be STRIPPED by the Jev
  mapping (protocol: no such field).
- Callers of `system_one` (grep): src/main.rs (3 CLI paths), src/wasm.rs:91,
  tests. No other production callers → adding the server execution layer does
  not disturb existing paths.

### `server-rs::request::parse_classifier_request` (src/server/request.rs:132-284)
- Pure over `&[u8]`; returns `Result<ClassifierRequest { model, context,
  questions: Vec<(String, Question)> in document order }, Vec<RequestError>>`.
- `Context` (request.rs:67-70): `State(Value)` | `Messages(Vec<ChatMessage>)`.
  JEV-005 needs the state as a JSON `Value` for `system_one`: `State(v)` → `v`;
  `Messages(m)` → `json!([{role, content}...])` (protocol-reference: the rlcd
  backend serializes messages as a JSON list of {role, content} objects).
- The 256-question schema cap is already enforced here; JEV-005 adds the
  configurable `--max-request-branches` (default 100) 422 on top.

### Crate-wide ordering constraint (Cargo.toml:36-44, lib.rs:11-12)
- `serde_json/preserve_order` is deliberately NOT enabled crate-wide (it would
  change prompt text via `schema::serialize_state`). Therefore
  `serde_json::Value::Object` is a **BTreeMap → sorted key serialization**.
  `agent::answer_to_json` (agent.rs:211-224) leans on this and emits
  `act_probability`. The Jev mapping must therefore build ordered JSON by
  **manual construction** (Vec of (key, value) pairs written to a string, or a
  dedicated ordered object type) for `answers.<qid>` objects and their
  `probabilities` maps — request insertion order is part of the protocol
  ("probabilities keyed in request order").
- Note: semantically JSON objects are unordered; the protocol conformance tests
  (JEV-007) compare parsed structure, but the feature file JEV-005 scenario 1
  pins "keyed in request order billing/technical" — build ordered to be safe
  and byte-deterministic.

### `server` module surface (src/server/mod.rs, lib.rs:15-16)
- `pub mod request` only so far; module is `#[cfg(not(target_arch = "wasm32"))]`.
  New files for JEV-005: `response.rs` (mapping + usage + envelope builders) and
  `state.rs` (ServerState + Answerer seam + admission). Both stay native-only.
- `RLAgent` is `pub use agent::RLAgent` at lib.rs:22; `Answer` re-exported.

## Test patterns to follow

- `tests/schema.rs`: weight-free sequence building with an in-memory WordLevel
  tokenizer (specials [unk]=0 [PAD]=1 [CLS]=2 [SEP]=3 [MASK]=4, words from id 5,
  Whitespace pre-tokenizer) — reuse this fixture pattern for the 422 admission
  tests (small `max_len` cap, long state → marker loss / overflow).
- `tests/agent.rs`: LAYA_TEST_MODEL-gated, `load_agent()` → early `return` skip
  when unset — the model-gated convention JEV-007 inherits.
- `tests/request_schema.rs` (JEV-003): `@step` comment convention, one test fn
  per Gherkin scenario, feature-file header comment naming the .feature path.

## Design consequences (feeding the architecture note)

1. `Answerer` trait seam in `src/server/state.rs` (`Send + Sync`):
   `system_one(&self, state: &Value, questions) -> anyhow::Result<Vec<(String, Answer)>>`
   + a method exposing per-question built sequence lengths for usage accounting
   (admission pre-builds the sequences with the effective cap; the same lengths
   feed `usage.input_tokens` — one build, two uses, no double tokenization).
   `Arc<RLAgent>` impl forwards to the agent; mock impl for weight-free tests.
   `ServerState` = `#[derive(Clone)] struct ServerState(Arc<Inner>)` with
   `model_lock: tokio::sync::Mutex<()>` + `admission: tokio::sync::Semaphore`
   (max_queued permits) — axum `State` extractor for JEV-004.
2. `answer_to_jev_json(&Answer, &Question) -> ordered Value` in response.rs:
   choice → {type, choice, confidence, probabilities{request-order keys}};
   score → {type, score, confidence, probabilities{"0".."N-1"},
   legend{same keys → criteria}}; noul → {type, noul}. No act_probability,
   no confidence on noul.
3. Admission (state.rs or response.rs helpers, pure over tokenizer access):
   422 before inference on (a) any question's natural sequence length >
   effective cap, (b) marker loss, (c) question count > max_request_branches —
   never touching the model.
4. Execution: try-acquire admission permit → full ⇒ 429 `{"detail":"Scoring
   queue is full"}` + `Retry-After: 1`; else model_lock →
   `spawn_blocking(answerer.system_one(...))` → map + usage → 200.
   Error ⇒ 500; disconnect during queued wait ⇒ drop / 499 if deliverable.
