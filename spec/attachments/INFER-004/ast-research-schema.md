# AST Research — Typed-question schema builder (INFER-004)

Generated via AstGrep over `src/schema.rs` during reverse ACDD discovery.

## `render_options` (src/schema.rs:43-65)

- `Choice`: each `choice_criteria` entry → `key` or `key: description` (empty description omitted), insertion order preserved.
- `Score`: each `score_criteria` entry → `level <i>: <criterion>` (0-based ordinal).
- `Noul`: exactly two options:
  - `false: <noul_false>` (default `"no, the statement does not hold"`)
  - `true: <noul_true>` (default `"yes, the statement holds"`)

## `serialize_state` (src/schema.rs:67-72)

- `Value::String(s)` → used verbatim.
- Anything else → `serde_json::to_string(other)` (compact).

## `build_sequence` (src/schema.rs:89-159)

Signature: `(tok, special, state, q, max_len, head_max_len) -> BuiltSequence { ids, markers }`.

Layout (lines 134-156), in order:
1. `ids.push(cls_id)` (line 135)
2. head = `<qtype> question: <instructions>` encoded, mask-char replaced with space (lines 103-105)
3. `ids.push(sep_id)` (line 137)
4. For each option: `markers.push(ids.len())` then extend with option tokens (lines 139-143)
5. `ids.push(sep_id)` (line 144) — after the options
6. state tokens appended, truncated to `room = max_len - ids.len() - 1` (lines 146-152)
7. `ids.push(sep_id)` (line 153) — after the state
8. `ids.truncate(max_len)` (line 155); `markers.retain(|&m| m < max_len)` (line 156)

Result: `[CLS] head [SEP] opt0 opt1 ... [SEP] state [SEP]`.

Per-option encoding (lines 107-115): option text = `" " + option (mask replaced)`, encoded, truncated to 48,
prepended with `mask_id` → full vector is `1 (mask) + up to 48 (text)`.

Head-budget fallback (lines 117-132):
- If `opt_budget = head_max_len - sum(option lens) < 16`:
  - `per = ((head_max_len - 16) / n).max(4)` — cap applied to each option's *full* vector (mask + text).
- `head_keep = opt_budget.max(8)`; head truncated to `head_keep`.

## `encode_ids` (src/schema.rs:161-163)

`tok.encode(text, false)` → `get_ids()`.

## Testability

- No model weights needed: `render_options` is pure string formatting; `build_sequence` takes a
  `tokenizers::Tokenizer`, which tests can construct in-memory (WordLevel model with known
  `[CLS]`/`[SEP]`/`[MASK]`/`[PAD]` token ids) — no checkpoint required.
