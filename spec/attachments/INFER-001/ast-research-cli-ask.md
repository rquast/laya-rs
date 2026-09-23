# AST Research — CLI `ask` subcommand (INFER-001)

Generated via AstGrep over `src/main.rs` during reverse ACDD discovery.

## Entry point

- `fn main() -> anyhow::Result<()>` — src/main.rs:95
  - Parses `Args` (clap `Parser`), dispatches on `Command` subcommand.

## Subcommand enum

- `enum Command { $$$ }` — src/main.rs:45
  - `Ask { model: Option<PathBuf>, state: String, question: String, options: Vec<String> }`
  - `Answer { input, output, model, model_variant, only }`
  - `Train { model_dir, dataset, epochs, lr, group_size, sigma, save_to }`

## `laya ask` handler (src/main.rs lines 98–121)

```
if let Some(Command::Ask { model, state, question, options }) = &args.command {
    let checkpoint = route(state);
    let dir = model_path::resolve(variant_key(checkpoint), model.clone(),
                                  Some(variant_key(checkpoint)),
                                  args.models_root.as_deref())?;
    eprintln!("routed to: {:?} ({})", checkpoint, dir.display());
    let agent = RLAgent::load(&dir)?;
    let q = Question { qtype: QType::Choice, instructions: question.clone(),
                       choice_criteria: options.iter().map(|o| (o.clone(), None)).collect(),
                       ... };
    let answers = agent.system_one(&json!(state), &[("answer".to_string(), q)])?;
    // prints `choice={choice} confidence={confidence:.4} act_p={act_probability:.4}`
    // plus one indented `{k}: {v:.4}` line per option
    return Ok(());
}
```

## Call chain

`main` → `Command::Ask` → `route()` (src/router.rs:106) → `model_path::resolve()` (src/model_path.rs:68)
→ `RLAgent::load()` (src/agent.rs:62) → `RLAgent::system_one()` (src/agent.rs:126).

## Testability notes

- `--state` / `--question` are required `String` args; `--option` is a repeatable `Vec<String>` (no clap `required`).
- `--model` / `LAYA_MODEL` bypass routing + resolution (explicit path used as-is).
- Answering requires checkpoint weights (not bundled in the repo) — integration tests gate on a
  `LAYA_TEST_MODEL` env var; arg-parsing behavior is testable without weights.
