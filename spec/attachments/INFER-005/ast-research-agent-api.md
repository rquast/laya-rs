# AST Research: Rust library API inference (`RLAgent` + `system_one`)

## Public API (src/agent.rs)

| Item | Signature | Notes |
|---|---|---|
| `RLAgent` | `pub struct RLAgent` (47) | `{ tok: Tokenizer, special: SpecialTokens, model: DecisionModel, cfg: RlAgentConfig, device: Device }` — all fields private |
| `RLAgent::load` | (62) | Reads `rl_agent_config.json`, `tokenizer/tokenizer.json`, `tokenizer/tokenizer_config.json`, `encoder/config.json`, `model.safetensors` (mmaped). `Device::cuda_if_available(0)`; F16 on CUDA (matches original `torch.autocast(fp16)`; 4-5x faster than F32 on GPU), F32 on CPU |
| `RLAgent::load_from_bytes` | (98) | Same five artifacts from in-memory bytes; always CPU/F32; weights via `crate::safetensors32::load_buffer` (the wasm path) |
| `RLAgent::system_one` | (126) | `&self, state: &Value, questions: &[(String, Question)] -> anyhow::Result<Vec<(String, Answer)>>`. One batched forward pass |
| `Answer` | `pub enum` (55) | `Choice { choice, probabilities: Vec<(String, f32)>, confidence, act_probability }` / `Score { score, legend, probabilities: Vec<f32>, confidence, act_probability }` / `Noul { noul, act_probability }` |
| `answer_to_json` | (211) | `{"type":"choice","choice","confidence","act_probability","probabilities":{...}}` / `{"type":"score","score","legend":{i:crit},"probabilities":{i:p}}` / `{"type":"noul","noul","act_probability"}` |

### system_one mechanics (lines 126-210)
1. Per question: `schema::build_sequence(&tok, &special, state, q, cfg.max_len, cfg.head_max_len)`; `if built.markers.len() != render_options(q).len() → bail!("question {qid:?}: options do not fit in head_max_len={}")` (line ~137).
2. Pad to batch `(b, max_l)` with `pad_id`; attention mask.
3. ONE `model.forward(&input_ids, &attention_mask, &all_markers, &qtypes)` → `(logits, act_probs)` (bidirectional — no autoregressive decode).
4. Temperature: `cfg.temperature_by_options[batching::temp_bucket(qtype, k)]` → `cfg.temperature[qtype.as_index()]` → `1.0`.
5. Softmax(logits/temp) → probabilities; `confidence = metrics::confidence_from_probs(&p)` (metrics.rs:29); `act_probability = act_probs[r][0]`.
6. Choice: argmax key + probs; Score: `Σ i·p_i` over `q.score_criteria` legend; Noul: `p[1]` (false-probability).

### Supporting
- `Question` (schema.rs:34): `qtype, instructions, choice_criteria: Vec<(String, Option<String>)>, score_criteria: Vec<String>, noul_true/false: Option<String>`; `render_options` (schema.rs) renders `key: value`, `level i: crit`, and noul true/false defaults.
- `temp_bucket` (batching.rs:343): `(qtype, option-count)` bucket key.
- `RlAgentConfig` fields incl. `temperature: Vec<f32>`, `temperature_by_options: HashMap<String, f32>`, `act_costs`, `max_len`, `head_max_len`, `head_layers`.

## Testability / gating
- Real inference needs a checkpoint (hundreds of MB) → gate on `LAYA_TEST_MODEL` (same pattern as cli_ask/cli_answer/cli_demo); skip when unset.
- Head-budget overflow: build >N options (N from `RLAgent` is not exposed → use a fixed generous count, e.g. 64 options of ~20 chars each, which exceeds any plausible `head_max_len` (default-era values ~128-256 tokens per option block) — deterministic failure, no model output involved; the error path happens before the forward pass, so this scenario does NOT need weights... but `load` still does (RLAgent::load runs first). Therefore all four scenarios gate on `LAYA_TEST_MODEL`.
- `answer_to_json` is pure (no model needed) — but the scenario says "answered one of each"; the JSON shape assertions run on real answers, gated like the rest.
