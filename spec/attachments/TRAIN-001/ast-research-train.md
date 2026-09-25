# AST Research: CLI RLCD training (`rlcd train`)

## Public API surface (src/train.rs)

| Item | Signature | Notes |
|---|---|---|
| `RlcdConfig` (struct) | `pub struct RlcdConfig` (line 58) | lr, group_size, noise_sigma, w_sph, w_rps, log_floor, td_lambda, act_loss_weight, max_tokens, max_seqs, seed. `Default`: lr 1e-5, group 8, sigma 1.0, w_sph 0.5, w_rps 1.0, log_floor -9.21, td_lambda 1.0, act_loss_weight 1.0, max_tokens 16384, max_seqs 256, seed 0 |
| `Trainer::load` | `pub fn load(model_dir, rlcd) -> anyhow::Result<Self>` (104) | Reads `rl_agent_config.json`, `tokenizer/tokenizer.json`, `tokenizer/tokenizer_config.json`, `encoder/config.json`, `model.safetensors` (varmap.load). CPU, F32, AdamW, seeded `StdRng::seed_from_u64(rlcd.seed)` |
| `Trainer::save` | `pub fn save(&self, path)` (130) | `varmap.save(path)` (safetensors) |
| `Trainer::train_step` | `pub fn train_step(&mut self, groups: Vec<Vec<Item>>, rlcd) -> anyhow::Result<(f32, f32)>` (137) | REINFORCE w/ Gaussian exploration; returns (loss, reward) |
| `Trainer::train_jsonl` | `pub fn train_jsonl(&mut self, path, epochs, rlcd) -> anyhow::Result<()>` (260) | Read file → `text.lines().filter(non-blank).map(serde_json::from_str).collect::<Result<_>>()` (parse errors propagate); per-epoch seeded shuffle, `order.chunks(32)`, `encode_record(..., true)` (option-order shuffling on), prints `epoch {epoch}: loss={:.4} reward={:.4} ({n_steps} steps)` |
| `fit_temperature` / `fit_temperatures` | (341 / 359) | Out of scope for TRAIN-001 (TRAIN-003) |

## CLI wiring (src/main.rs)

- `Command::Train { model_dir, dataset, epochs (default 1), lr (default 1e-5), group_size (default 8), sigma (default 1.0), save_to: Option<String> }` (lines ~78-89)
- Handler at line 155-162: builds `RlcdConfig { lr, group_size, noise_sigma: sigma, ..Default }`, `Trainer::load(&model_dir)`, `trainer.train_jsonl(&dataset, epochs, &rlcd)`, `out = save_to.unwrap_or_else(|| "{model_dir}/model.trained.safetensors")`, `trainer.save(&out)`, prints `saved trained weights to {out}`.

## Dataset schema (src/batching.rs)

- `Record` (untagged): `Episode { kind, ep: Episode, qs: Vec<RawQuestion>, src }` | `Plain { state: Value, qs: Vec<QuestionWithLabel>, src }`
- `RawQuestion { type (t), instructions, criteria? }`; `QuestionWithLabel { flatten RawQuestion, y: Option<usize>, soft: Option<Vec<f32>> }`
- `raw_question_to_question` panics on unknown question type (`"choice"|"score"|"noul"`).
- Blank JSONL lines are skipped; a non-JSON line → serde error → `collect::<Result<_>>` fails → `train_jsonl` returns Err → CLI exits non-zero with the parse error on stderr.

## Test determinism / gating

- `train_jsonl` uses seeded RNG (seed from RlcdConfig, default 0) → deterministic shuffling.
- Training requires a real checkpoint dir (`Trainer::load` before dataset read) → tests gate on `LAYA_TEST_MODEL` (same pattern as cli_ask/cli_answer/cli_demo tests) and skip when unset.
