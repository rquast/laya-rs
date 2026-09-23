# AST Research — Jev classifier request schema (JEV-003)

Direct source analysis of the existing question-parsing and sequence-building
code that `src/server/request.rs` (the strict protocol request schema) must
reuse or diverge from.

## Existing question shape (src/batching.rs)

- `RawQuestion` (lines 56–62): `type` (renamed from `type`), `instructions:
  Value`, `criteria: Option<Value>` — an untyped envelope.
- `raw_question_to_question(&RawQuestion) -> Question` (lines 73–109):
  - `type`: "choice" | "score" | "noul"; **unknown type → `panic!`** (line 78).
  - instructions: `Value::String` passed through; anything else is
    `to_string()` (line 83) — i.e. serde's default pretty JSON, NOT the
    protocol's canonical().
  - choice criteria: JSON object `{key: description?}` keeping insertion
    order, OR an array of strings (lines 93–98); non-string descriptions are
    silently dropped (`v.as_str().map(...)`).
  - score criteria: array, `filter_map(as_str)` — non-string entries dropped.
  - noul criteria: object, only `true`/`false` keys read via `as_str`
    (lines 102–105); **unknown keys silently ignored**.
  - No cardinality checks, no unknown-field rejection, no context XOR.

→ The protocol's strictness (422s, canonical(), 2–50 caps) is a *new* layer;
`raw_question_to_question` is too permissive and panics on unknown types.

## Target types (src/schema.rs)

- `QType` (line 8): Choice/Score/Noul + `as_index`/`as_str`.
- `Question` (line 34): `qtype: QType`, `instructions: String`,
  `choice_criteria: Vec<(String, Option<String>)>`, `score_criteria:
  Vec<String>`, `noul_true/noul_false: Option<String>` — all public,
  Clone+Debug.
- `render_options(&Question) -> Vec<String>` (line 43): choice →
  `key` or `key: description`; score → `level {i}: {c}`; noul →
  `[false: {f}, true: {t}]` with defaults
  "no, the statement does not hold" / "yes, the statement holds".
- `serialize_state(&Value) -> String` (line 67): `String` passthrough, else
  `serde_json::to_string` (compact, unsorted — state is context, not an entry;
  the protocol accepts that the state serialization is server-defined, but
  `messages` are serialized as a JSON list — the reference laya backend
  serializes them as a list of role/content objects, which `serde_json`
  compact form matches).

## Sequence builder (consumed downstream, unchanged)

- `build_sequence(tok, special, state, q, max_len, head_max_len) ->
  BuiltSequence { ids, markers }` (line 89): `[CLS] <type> question:
  <instructions> [SEP] [MASK] opt0 [MASK] opt1 ... [SEP] <state> [SEP]`,
  per-option 48-token cap, head-budget fallback (opt_budget < 16 → per-option
  cap `(head_max_len-16)/n`, floor 4), state truncated to remaining room,
  final `truncate(max_len)` + `markers.retain(|m| m < max_len)`.
- Marker loss on truncation is detectable by the caller:
  `built.markers.len() != render_options(q).len()` — already the bail used in
  `agent.rs:136-138` ("options do not fit in head_max_len"). The server
  pre-admission (JEV-005) reuses exactly this check against
  `min(--max-model-len, cfg.max_len)`.

## Error-surface precedent

- `agent.rs` bails with `anyhow!` on marker loss; `model_path`/`download`
  return `anyhow::Result` with readable messages. The protocol layer
  (JEV-003) needs structured multi-error accumulation (`Vec<RequestError>`),
  not single-error anyhow, because the 422 envelope carries up to 10
  detail entries.

## Testability notes

- `parse_classifier_request` is pure over `&[u8]` → fully unit-testable,
  weight-free (the `tests/schema.rs` in-memory WordLevel tokenizer pattern is
  NOT needed here — no tokenization at this layer).
- `laya::Question` is constructible directly in tests (public fields).
- Non-string entry canonicalization is testable by comparing against the
  expected compact sorted-key JSON strings.
