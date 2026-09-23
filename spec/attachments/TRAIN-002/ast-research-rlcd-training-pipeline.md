# AST Research: TRAIN-002 (RLCD training loop)

Discovery-phase code research for `spec/features/rlcd-training-loop.feature`.
Scope: `src/batching.rs` (pure pipeline), `src/train.rs` (Trainer + helpers),
`src/schema.rs` (sequence building), `src/metrics.rs` (reward).

## Batch pipeline (src/batching.rs) — pure, unit-testable

| Entity | Location | Role |
|---|---|---|
| `pub struct Item` | src/batching.rs:15-25 | model-ready item: `ids`, `markers`, `qtype`, soft `target`, `label` (-1 unlabeled), `episode`, `ep_step`, `ep_len`, `rec_uid` |
| `pub enum Record` (untagged) | src/batching.rs:29-46 | `Episode{kind, ep: Episode, qs: Vec<RawQuestion>}` or `Plain{state: Value, qs: Vec<QuestionWithLabel>}` |
| `pub struct Episode` | src/batching.rs:49-53 | `ctx: Value`, `turns: Vec<Value>`, `y: f32` |
| `pub struct RawQuestion` | src/batching.rs:56-62 | `t` (renamed `type`), `instructions: Value`, `criteria: Option<Value>` |
| `pub struct QuestionWithLabel` | src/batching.rs:65-71 | flattened `RawQuestion` + `y: Option<usize>` + `soft: Option<Vec<f32>>` (JSON key `soft`) |
| `raw_question_to_question` | src/batching.rs:73-109 | `choice/score/noul` -> `Question`; unknown type **panics**; choice criteria from object or array |
| `pub fn episode_prefix_lengths(n_turns, max_prefixes)` | src/batching.rs:112-122 | `1..=n` when `n <= max_prefixes`; else `max_prefixes` evenly spaced rounded lengths (BTreeSet dedup, sorted) |
| `pub fn encode_record(tok, special, rec, max_len, head_max_len, max_prefixes, rec_uid, rng, train)` | src/batching.rs:125-208 | Episode: one Noul item per prefix length, target `[1-y, y]`, `label = y.round()`, state gets `conversation` = `turns[..t]`; markers must be exactly 2 else skipped. Plain: one item per question; non-score options shuffled via `rng` when `train`; soft target or one-hot `y`; target/label remapped into shuffled order; item dropped when markers < option count |
| `reorder_question` (private) | src/batching.rs:210-218 | applies option `order` to choice criteria; score/noul untouched |
| `pub struct Collated` | src/batching.rs:221-232 | row-major `ids [n,l]` (pad id elsewhere), `attention_mask [n,l]`, `marker_pos [n,kmax]`, `marker_mask [n,kmax]` (1.0 real), `target [n,kmax]`, `qtype [n]`, `label [n]` |
| `pub fn collate_items(items: &[&Item], pad_id)` | src/batching.rs:234-261 | pads to `l = max ids len`, `kmax = max marker count`; qtype via `QType::as_index` (choice=0, score=1, noul=2) |
| `pub fn pack_groups(groups, max_tokens, max_seqs)` | src/batching.rs:265-300 | drops empty groups; sorts by group max item len; a group whose own `g_max*g_n > max_tokens` is split alone into `step = max(1, max_tokens/g_max)`-sized sub-batches; otherwise groups are merged while `new_max*new_n <= max_tokens && new_n <= max_seqs` |
| `pub fn make_token_batches(lengths, nseq, max_tokens, max_seqs, rng, chunk)` | src/batching.rs:304-340 | shuffles record order, chunks by `chunk` (32 in train_jsonl), length-sorts each chunk, greedily starts a new batch when `new_max*new_n > max_tokens || new_n > max_seqs`, shuffles the final batch list; every index exactly once |
| `pub fn temp_bucket(qtype, k)` | src/batching.rs:343-346 | `"choice:2"`, `"score:3-5"` etc. (used by fit_temperatures) |
| `pub fn td_lambda_targets(items, p_true, lam)` | src/batching.rs:351-371 | per `rec_uid` episode group, sorted by `ep_step`: backward walk `g_j = (1-lam)*p_true[next] + lam*g_{j+1}`, final prefix keeps its raw target; non-episode items untouched |

## Trainer (src/train.rs)

| Entity | Location | Role |
|---|---|---|
| `pub struct RlcdConfig` | src/train.rs:58-88 | lr 1e-5, group_size 8, noise_sigma 1.0, w_sph 0.5, w_rps 1.0, log_floor -9.21, td_lambda 1.0, act_loss_weight 1.0, max_tokens 16384, max_seqs 256, seed 0 |
| `pub struct Trainer` | src/train.rs:90-101 | tok, special, `DecisionModel`, varmap, AdamW, cfg, `act_costs: Vec<f64>`, device, `StdRng`, `step` |
| `Trainer::load(model_dir, rlcd)` | src/train.rs:104-128 | reads `rl_agent_config.json` (head_layers, max_len, head_max_len, max_prefixes, act_costs), `tokenizer/tokenizer.json` + `tokenizer_config.json` (cls/sep/mask/pad), `encoder/config.json`; `DecisionModel::load`; `varmap.load(model.safetensors)` |
| `Trainer::train_step(groups, rlcd)` | src/train.rs:137-255 | per sub-batch: collate -> `forward_tensors` -> TD(lambda) targets for episode items (clean masked-softmax P(true)) -> sample `g` noisy rows (N(0, sigma^2) on real option slots only) -> `proper_reward` per row (score questions get RPS term) -> group-mean baseline -> advantage -> REINFORCE loss = mean of `sum_c((x - c_mu)^2 / (2 sigma^2)) * advantage` over rows (grad through live logits) -> act-head weighted-BCE auxiliary (target 1 when clean argmax != label; weight `max(act_costs.first(), 0.1)` for wrong, 1.0 for right, 0 for unlabeled) -> AdamW `backward_step`. Returns (mean total loss, mean reward) over items |
| `Trainer::train_jsonl(path, epochs, rlcd)` | src/train.rs:260-304 | parse JSONL (blank lines skipped, parse errors propagate), per-epoch shuffle with seeded RNG, 32-record chunks, `encode_record` with shuffling enabled, `train_step` per chunk, prints `epoch {n}: loss={:.4} reward={:.4} ({steps} steps)` |
| `softmax_slice` (private) | src/train.rs:307-312 | stable max-subtract softmax |
| `masked_softmax` (private) | src/train.rs:314-323 | per-row softmax over real marker slots |
| `weighted_cross_entropy` (private) | src/train.rs:326-336 | BCE over act logits, weight-normalized |
| `fit_temperature` / `fit_temperatures` (pub) | src/train.rs:341-386 | post-hoc per-`temp_bucket` temperature fitting (covered by TRAIN-003 / temperature-calibration.feature) |

## Supporting pieces

- `src/metrics.rs:6` `proper_reward(q, target, is_score, w_sph, w_rps, log_floor)` — strictly-proper scoring rule (log + spherical; + ranked-probability penalty for score questions), log terms floored at `log_floor`. Used by train_step's reward; also exercised by INFER-005.
- `src/schema.rs` — `render_options` (noul always exactly `[false, true]`), `build_sequence` ([CLS] head [SEP] [MASK] opt [MASK] opt ... [SEP] state [SEP]; markers retained only when `< max_len`). The in-memory WordLevel tokenizer pattern from `tests/schema.rs:22-46` is the established test harness for weight-free pipeline tests.

## Test-surface decision

All six scenarios map to **pure functions** in `src/batching.rs`:

| Scenario | Functions under test |
|---|---|
| Collate pads mixed-length items | `collate_items` |
| Option shuffling remaps targets/labels | `encode_record` (Plain, train=true, seeded RNG) |
| Record groups stay together under budget | `pack_groups` |
| Token batches respect padded-token budget | `make_token_batches` |
| Episode records yield one item per prefix | `encode_record` (Episode), `episode_prefix_lengths` |
| Episode targets bootstrapped with TD lambda | `td_lambda_targets` |

`Trainer::load`/`train_step`/`train_jsonl` require a real checkpoint and stay
behind `LAYA_TEST_MODEL` (already covered by tests/train.rs for the CLI path);
no new model-dependent tests are needed for this work unit.

## Callers

- `src/main.rs:156-158` — `laya train` CLI builds `RlcdConfig`, `Trainer::load`, `train_jsonl`.
- `src/train.rs` — `train_jsonl` -> `encode_record` + `train_step`; `train_step` -> `pack_groups` -> `collate_items` -> `td_lambda_targets`.
- No production callers of `make_token_batches` currently (it is the budget-aware
  length-bucketing counterpart of `pack_groups` and is pinned by tests directly);
  `train_jsonl` itself chunks the shuffled record order via `order.chunks(32)`.
