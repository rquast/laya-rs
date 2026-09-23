# AST Research: INFER-006 (wasm browser bindings)

Discovery-phase code research for `spec/features/wasm-browser-agent.feature`
(re-scoped to the single compile-criterion scenario).

## WasmAgent surface (src/wasm.rs, wasm32-only, lib.rs:17-18 `pub mod wasm`)

| Entity | Location | Role |
|---|---|---|
| `fn to_js_err` | src/wasm.rs:19-21 | `Display -> JsValue::from_str` (plain message) |
| `fn anyhow_to_js_err` | src/wasm.rs:25-27 | `anyhow::Error -> JsValue` via `format!("{e:#}")` — full context chain |
| `#[wasm_bindgen(start)] fn init_panic_hook` | src/wasm.rs:31-36 | `console_error_panic_hook::set_once()` |
| `pub struct WasmAgent { inner: RLAgent }` | src/wasm.rs:40-42 | opaque handle |
| `WasmAgent::load(weights, tokenizer, tokenizer_config_json, encoder_config_json, rl_agent_config_json) -> Result<WasmAgent, JsValue>` | src/wasm.rs:56-78 | delegates to `RLAgent::load_from_bytes` (CPU/F32), errors via `anyhow_to_js_err` |
| `WasmAgent::ask(state_json, questions_json) -> Result<String, JsValue>` | src/wasm.rs:82-94 | parse state JSON, `BTreeMap<String, RawQuestion>`; empty -> `Err("no questions given")`; `system_one`; `answer_to_json` per qid; serialize `{qid: {...}}` |

## Shared logic (already covered natively — why the harness was dropped)

- `RLAgent::load_from_bytes` — src/agent.rs:98 (also exercised by tests/agent.rs)
- `RLAgent::system_one` — src/agent.rs:126
- `answer_to_json` — src/agent.rs:211 (JSON shape asserted in tests/agent.rs)
- `raw_question_to_question` — src/batching.rs:73 (schema tests)
- empty-batch guard — identical pattern in `laya answer` (src/main.rs, `ensure!(!questions.is_empty(), "no questions to answer")`), pinned by tests/cli_answer.rs

## Build wiring (the criterion this feature now verifies)

- `Cargo.toml` lib target: `crate-type = ["rlib", "cdylib"]`
- `tokenizers = { version = "0.20", default-features = false, features = ["unstable_wasm"] }`
  — pure-Rust `fancy-regex` backend for ALL targets; `onig` (C-linked) is the
  default-features backend and is therefore never in the tree
- wasm32-only deps (Cargo.toml `[target.'cfg(target_arch = "wasm32")'.dependencies]`):
  wasm-bindgen 0.2, js-sys, serde-wasm-bindgen, console_error_panic_hook,
  getrandom(js), getrandom 0.3 (wasm_js)
- Consumer: `website/src/lib/laya.ts` fetches the five checkpoint files from HF
  and calls `WasmAgent.load`/`ask`; the shipped artifact
  `website/src/wasm-pkg/` (wasm-bindgen `--target web` output) exposes
  `wasmagent_load [i32 x10] -> [i32, i32, i32]` and
  `wasmagent_ask [i32 x5] -> [i32 x4]` (verified via wasmtime ABI probe)

## Decision (product): no wasm VM in the test path

The wasmtime harness (reimplementing the module's 28 `./laya_bg.js` imports)
was dropped as disproportionate: the only wasm-specific, cheap, high-value
check is that the lib compiles for wasm32-unknown-unknown with no C-linked
tokenizer backend — now pinned by `tests/wasm_compile.rs`
(`cargo build --target wasm32-unknown-unknown --lib` + `cargo tree` onig check).
Runtime load/ask behavior is exercised by the browser demo (website).
