# AST Research: checkpoint directory resolution (`model_path::resolve`)

## Public API (src/model_path.rs, native-only `#[cfg(not(target_arch = "wasm32"))]`)

| Item | Signature | Notes |
|---|---|---|
| `VariantDef` | `pub struct VariantDef { key, hf_repo, subfolder }` (line 28) | One targetable checkpoint |
| `VARIANTS` | `pub const VARIANTS: &[VariantDef]` (35) | `typed-decisions` (hf `convaiinnovations/laya-typed-decisions`, subfolder `typed-decisions`), `multilingual` (hf `convaiinnovations/laya-multilingual`, subfolder `multilingual`) |
| `find` | `pub fn find(key) -> Result<&'static VariantDef>` (40) | `unknown model variant {key:?}` context on miss |
| `resolve` | `pub fn resolve(default_variant_key, explicit: Option<PathBuf>, requested_variant: Option<&str>, family_root: Option<&Path>) -> Result<PathBuf>` (68) | Pure order logic — see below |

### Resolution order (resolve, lines 68-88)
1. `explicit` (from `--model`/`LAYA_MODEL`) → returned as-is; no variant lookup, no family-root, no cache, no download.
2. `variant = find(requested_variant.unwrap_or(default_variant_key))` → Err `unknown model variant`.
3. `family_root` (from `--models-root`/`LAYA_MODELS_ROOT`; no default): `find_in_family_root` — candidates in order `hf_repo` basename (`laya-typed-decisions`) then `subfolder` (`typed-decisions`); first dir containing `model.safetensors` wins.
4. `crate::download::download_variant(variant)` → cache path (may download).

## Download side (src/download.rs, native-only)

- `cache_dir()` (22): `$XDG_CACHE_HOME` (if absolute) else `$HOME/.cache`, + `/laya-rs`.
- `variant_cache_dir(variant)` (37): cache dir + `hf_repo` basename (e.g. `laya-multilingual`).
- `REQUIRED_FILES` (18): `rl_agent_config.json`, `encoder/config.json`, `tokenizer/tokenizer.json`, `tokenizer/tokenizer_config.json`, `model.safetensors`.
- `download_variant` (86): create dir; per file — skip if present; if `LAYA_OFFLINE` set and file missing → `bail!("{file} is missing from {dir} and LAYA_OFFLINE is set; unset it to allow downloading, or pass --model")`; else fetch `https://huggingface.co/{hf_repo}/resolve/main/{file}` → `.part` temp + rename (atomic), error leaves no partial that looks complete.

## Callers (wiring)

- `src/main.rs` Ask arm: `route(state)` → `model_path::resolve(variant_key(checkpoint), model, Some(variant_key(checkpoint)), args.models_root)`.
- `src/main.rs` Answer arm: `model_path::resolve(model_variant, model, Some(model_variant), args.models_root)`.
- CLI flags: `--model` (env LAYA_MODEL), `--models-root` (env LAYA_MODELS_ROOT), `--model-variant` (Answer, default `typed-decisions`).

## Test plan constraints

- `resolve`/`find` are pure fs+env logic: explicit + family-root + unknown-variant scenarios are fully testable offline.
- Cache scenarios: `download_variant` consults `XDG_CACHE_HOME` (env) — tests point it at a temp dir via `std::env::set_var` (note: tests in one process share env; use distinct temp dirs per test; cache-complete scenario needs 5 files; LAYA_OFFLINE scenario needs them absent — env mutation must be scoped carefully, e.g. run in separate test binary or set/restore per test).
