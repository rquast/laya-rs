# AST Research — CLI `answer` subcommand (INFER-002)

Generated via AstGrep over `src/main.rs` + `src/batching.rs` + `src/agent.rs` during reverse ACDD discovery.

## Subcommand definition (src/main.rs:66-77)

```rust
Answer {
    input: String,
    output: String,
    #[arg(long, env = "LAYA_MODEL")]
    model: Option<PathBuf>,
    #[arg(long, default_value = "typed-decisions")]
    model_variant: String,
    #[arg(long)]
    only: Option<String>,
}
```

## Handler (src/main.rs lines 123–153)

- Reads `input` file, parses JSON; `raw.get("state")` → `anyhow::ensure!`-style error `missing state`.
- `raw.get("questions").and_then(as_object)` → error `missing questions`.
- Iterates `questions_obj`; `--only` filters by qid; `serde_json::from_value::<RawQuestion>` → `raw_question_to_question`.
- `anyhow::ensure!(!questions.is_empty(), "no questions to answer")`.
- Resolves checkpoint: `model_path::resolve(model_variant, model.clone(), Some(model_variant), models_root)`.
- `RLAgent::load(&dir)` → `agent.system_one(&state, &questions)`; times the call, prints
  `{} questions in {elapsed_ms:.2} ms ({:.2} ms/question)` to stderr.
- Builds `BTreeMap<String, Value>` via `answer_to_json`; writes `serde_json::to_string_pretty` to `output`.

## Question shape mapping (src/batching.rs `raw_question_to_question`, lines 73–109)

- `type`: "choice" | "score" | "noul" (unknown → `panic!`).
- choice: `criteria` object `{key: description?}` (insertion order) or array of strings.
- score: `criteria` array of level strings (ordinal).
- noul: `criteria` object `{"true": .., "false": ..}` (optional; defaults in `render_options`).

## Answer JSON shape (src/agent.rs `answer_to_json`, lines 211–224)

- choice: `{type, choice, confidence, act_probability, probabilities: {option: p}}`
- score: `{type, score, confidence, act_probability, legend: {i: level}, probabilities: {i: p}}`
- noul: `{type, noul, act_probability}`

## Contract mismatch noted

`scripts/jev_batch.py` invokes `rlcd answer ... --model-dir DIR`, but the `Answer` subcommand defines
`--model` (env `LAYA_MODEL`), not `--model-dir`. Tracked as a red card under INFER-007.

## Testability notes

- Validation paths (missing state/questions, unknown type, empty after `--only`) are testable without weights.
- Batch answering requires a checkpoint (LAYA_TEST_MODEL env var); weights not bundled.
