@done
@INFER-006
@decision-inference
@decision-engine
Feature: Answer typed questions in the browser via wasm bindings
  """
  src/wasm.rs (wasm32-only module, lib.rs line 17-18): WasmAgent { inner: RLAgent };
  load(weights, tokenizer, tokenizer_config_json, encoder_config_json, rl_agent_config_json)
  -> Result<WasmAgent, JsValue> (CPU/F32 via RLAgent::load_from_bytes +
  safetensors32::load_buffer); ask(state_json, questions_json) -> Result<String, JsValue>
  (parses state as arbitrary JSON, questions as BTreeMap<String, RawQuestion>,
  empty -> Err "no questions given", system_one, answer_to_json per qid,
  serializes {qid: {...}}). Errors: anyhow_to_js_err uses format!("{e:#}") for the
  full context chain; to_js_err for plain. #[wasm_bindgen(start)] init_panic_hook
  (console_error_panic_hook). Callers: browser JS (e.g. a page script) fetches the five
  files from HF and calls load/ask; lib is crate-type [rlib, cdylib].

  SCOPE NOTE: the load/ask business logic (empty-batch rejection, answer JSON
  shape, error context surfacing) is shared with the native CLI path and is
  covered by the native test files (tests/schema.rs, tests/cli_answer.rs,
  tests/agent.rs). Running the wasm in a real VM is intentionally NOT part of the
  test path (deferred product decision); the browser (cdylib) path exercises it
  end-to-end. This feature verifies the wasm-specific criterion: the lib builds
  for the browser target with the onig-free pure-Rust tokenizers backend.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The lib must compile for wasm32-unknown-unknown with the pure-Rust
  #      `unstable_wasm` tokenizers backend (no `onig` C linkage)
  #
  # EXAMPLES:
  #   1. `cargo build --target wasm32-unknown-unknown` succeeds for the lib,
  #      with no C-linked tokenizer backend in the dependency tree
  #
  # ========================================
  Background: User Story
    As a Browser/JS developer
    I want to run rlcd in the browser via WasmAgent (load checkpoint from bytes, ask a batch of typed questions)
    So that I get the same typed decisions in the browser demo without a Python backend

  Scenario: The crate compiles for the browser target
    Given the rlcd library with its pure-Rust tokenizers backend
    When I compile the lib for wasm32-unknown-unknown
    Then the build succeeds with no C-linked tokenizer backend in the dependency tree
