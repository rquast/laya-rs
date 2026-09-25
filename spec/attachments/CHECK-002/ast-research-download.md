# AST Research: checkpoint download (`download::download_variant`)

## Public API (src/download.rs, native-only `#[cfg(not(target_arch = "wasm32"))]`)

| Item | Signature | Notes |
|---|---|---|
| `REQUIRED_FILES` | `const REQUIRED_FILES: &[&str]` (18) | `rl_agent_config.json`, `encoder/config.json`, `tokenizer/tokenizer.json`, `tokenizer/tokenizer_config.json`, `model.safetensors` (mirrored by the browser-demo convention) |
| `cache_dir()` | (22) | `$XDG_CACHE_HOME` (must be absolute) else `$HOME/.cache`; returns `$base/rlcd-rs`; None if neither set |
| `variant_cache_dir(v)` | (37) | cache dir + `hf_repo` basename (`laya-typed-decisions` / `laya-multilingual`) |
| `download_variant(v)` | (86) | `create_dir_all(dir)`; per file: skip if `dest.is_file()`; first missing file + `RLCD_OFFLINE` set → `bail!("{file} is missing from {dir} and RLCD_OFFLINE is set; unset it to allow downloading, or pass --model")`; else `fetch_to_file`. Returns dir |
| `fetch_to_file(url, dest)` (private, 40) | ureq 2 (blocking) GET → `.part` sibling (`ext.part`); 64 KiB buffered reads; if `Content-Length` advertised and `written != len` → delete `.part` + `bail!("short read for {url}: got {written} bytes, expected {len}")`; `rename(.part → dest)` (atomic); stderr progress `<name> ... done (N bytes)` |

## Network semantics

- URL: `https://huggingface.co/{hf_repo}/resolve/main/{file}`.
- ureq 2 = synchronous, no async runtime; native-only (Cargo.toml line 63; wasm target instead uses the browser demo's JS `fetch`).
- Failure modes: HTTP error → `downloading {url}` context; short read → `.part` removed + bail; rename failure → context `renaming ...`.

## Testability

- `download_variant` consults `XDG_CACHE_HOME` (env) at call time → tests can point it at temp dirs (env mutation needs ENV_LOCK as in tests/model_path.rs).
- Fully-cached scenario: deterministic offline (RLCD_OFFLINE set + all files present → skip loop, no fetch).
- Partial+offline scenario: deterministic error, names first missing file + `RLCD_OFFLINE` + `--model`.
- Fresh-cache real download: ~300 MB from HF — gated behind `RLCD_TEST_DOWNLOAD` env (skip otherwise), same opt-in pattern as `LAYA_TEST_MODEL`.
