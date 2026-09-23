# AST Research: temperature calibration (`fit_temperature` / `fit_temperatures`)

## Public API (src/train.rs — pure, weight-free, no I/O)

| Item | Signature | Notes |
|---|---|---|
| `fit_temperature` | `pub fn fit_temperature(logits: &[f32], target_index: usize) -> f32` (line 341) | Single-sample grid search: candidates `0.05..=20.0` step `0.05` (400 values: `i*0.05, i in 1..=400`); score = NLL `-p[target].ln()` with `p = softmax_slice(logits/t)` and `p` clamped at `1e-12`; argmin; default best `1.0` (only if no candidate beats INFINITY, i.e. never) |
| `fit_temperatures` | `pub fn fit_temperatures(samples: &[(QType, Vec<f32>, usize)]) -> HashMap<String, f32>` (line 359) | Groups samples by `batching::temp_bucket(qtype, logits.len())`; per bucket: argmin of MEAN NLL over the same 400-candidate grid; returns `bucket -> best_t` |
| `softmax_slice` | `fn softmax_slice(logits: &[f32]) -> Vec<f32>` (line 307, private) | Numerically-stable-ish softmax (max-shift) |
| `temp_bucket` | `pub fn temp_bucket(qtype: QType, k: usize) -> String` (batching.rs:343) | Bucket size: `k<=2 -> "2"`, `k<=5 -> "3-5"`, `k<=10 -> "6-10"`, else `"11+"`; key = `"{qtype}:{size}"` (e.g. `choice:2`, `score:3-5`, `noul:2`) |

## Deterministic test vectors (verified by hand / python)

- `[4.0, -4.0]`, target 0 (argmax) → best `t = 0.05` (hottest sharpening already maximizes p_target)
- `[4.0, -4.0]`, target 1 (non-argmax) → best `t = 20.0` (flattening maximizes p_target; NLL → 0.913 at t=20)
- `[3.0, -1.0, -2.0]`, target 0 → `0.05`; target 2 → `20.0` (three options → bucket `choice:3-5`)
- Two samples both `[4,-4]` target 0 in `choice:2` → shared `0.05`
- Mixed `choice:2` (t→0.05) + `choice:3-5` (t→20.0) → `{"choice:2": 0.05, "choice:3-5": 20.0}`

## Consumers (wiring)

- Runtime: `RLAgent::system_one` (agent.rs:~171) looks up `cfg.temperature_by_options[batching::temp_bucket(qtype, k)]` → falls back to `cfg.temperature[qtype.as_index()]` → `1.0`.
- The map produced by `fit_temperatures` is what gets written into `rl_agent_config.json`'s `temperature_by_options` (post-hoc calibration step, README: per-qtype fitting cuts expected calibration error 0.466 → 0.081).

## Test plan

- No checkpoint / network / env needed: pure functions on `&[(QType, Vec<f32>, usize)]`.
- `QType` values: `choice`, `score`, `noul` (as_str).
- Tests: tests/temperature.rs — 4 scenarios, no gating.
