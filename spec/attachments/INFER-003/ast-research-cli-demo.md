# AST Research — CLI demo (no-subcommand) path (INFER-003)

Generated via AstGrep over `src/main.rs` during reverse ACDD discovery.

## Default handler (src/main.rs lines 165–237)

Executes only when no `ask`/`answer`/`train` subcommand was parsed.

1. `let checkpoint = route(&args.body);` (line 165) — routes the positional body.
2. `model_path::resolve(variant_key(checkpoint), None, Some(variant_key(checkpoint)), args.models_root.as_deref())` —
   `--model`/`LAYA_MODEL` can still point at an explicit directory (lines 36–37 global flag).
3. `println!("routed to: {:?} ({})", checkpoint, model_dir.display());` — goes to stdout (note: the `ask` path uses `eprintln!` instead).
4. `RLAgent::load(&model_dir)?`.
5. Demo state: `let state = json!({ "subject": "Duplicate charge on invoice 4411", "body": args.body });` (line 171).
6. Hard-coded battery (lines 176–214):
   - `department`: choice {billing, technical, sales} with criteria descriptions.
   - `urgency`: score [not urgent, soon, blocking].
   - `churn_risk`: noul.
7. `agent.system_one(&state, &questions)` then a `match` over `rlcd::Answer` prints:
   - `Choice` → `{qid}: choice={choice} confidence={confidence:.4} act_p={act_probability:.4}` + indented `{k}: {v:.4}`.
   - `Score` → `{qid}: score={score:.4} confidence={confidence:.4} act_p={act_probability:.4}` + indented legend lines.
   - `Noul` → `{qid}: noul={noul:.4} act_p={act_probability:.4}`.

## Positional argument (src/main.rs lines 40–41)

```rust
#[arg(default_value = "We were billed twice for March. Please refund the duplicate.")]
body: String,
```

## Testability notes

- Requires a checkpoint (LAYA_TEST_MODEL env var): the model loads before any output.
- Weights are not bundled in the repo; both scenarios are model-gated in the integration test.
