# Agent Development Guidelines for rlcd-rs

This document provides guidelines for AI assistants working on the **rlcd-rs codebase**. This is about DEVELOPING rlcd-rs itself, not using it.

---

## Project Overview

**rlcd-rs** is a pure-Rust (candle) reimplementation of Laya, the sub-35ms non-autoregressive "System 1" decision engine. It serves typed-decision questions (choice/score/noul) over the Jev protocol — as a CLI, a tokio/axum HTTP server, a wasm browser agent, and a training loop (RLCD: REINFORCE with Gaussian exploration and a group-mean baseline).

- **Repository**: https://github.com/apiplant/rlcd-rs
- **License**: Apache-2.0
- **Language**: Rust (single crate at the repo root; `src/` + `tests/`)

For complete project context:
- **Project foundation**: [spec/FOUNDATION.md](spec/FOUNDATION.md)
- **Acceptance criteria**: `spec/features/*.feature` (Gherkin, managed with `fspec`)
- **Tags**: `spec/TAGS.md`

---

## ⚠️ Build Memory: OOM Guard (read this first)

This machine has **20 cores**, so cargo defaults to `-j 20`. The candle
dependency graph (candle-core, candle-nn, tokenizers, half) is heavy
enough that 20 concurrent rustc/link processes **OOM the box** on both
`cargo build` and `cargo test` — each of the ~17 integration-test
binaries in `tests/` statically links the full candle crate graph, and a
full run also triggers a nested `cargo build --target
wasm32-unknown-unknown` from inside `tests/wasm_compile.rs`. CI never
hits this because its 2–4 vCPU ceiling bounds peak memory.

**.cargo/config.toml sets `build.jobs = 4`** and applies to every cargo
invocation in this repo (build, test, check, clippy). Do not remove it.

- Override for faster builds on machines with headroom:
  `CARGO_BUILD_JOBS=8 cargo test --test agent`
- Precedence (highest wins): `-j <n>` flag → `CARGO_BUILD_JOBS` env → config file

### ⚠️ NEVER run an unscoped `cargo test` for a full check

A plain `cargo test` compiles all ~17 integration-test binaries (full
DWARF debug info, each linking the whole candle graph) **and** runs the
nested wasm build. On a memory-constrained box that is how the machine
crashes.

**Safe invocation patterns:**

```bash
# 1. Scope by target (preferred for development):
cargo test --test agent
cargo test --test jev_conformance

# 2. A broader but still explicit run (no --no-fail-fast surprises):
cargo test --test agent --test schema --test train

# 3. Unit tests only (no integration binaries):
cargo test --lib
```

A full `cargo test` is acceptable **only** deliberately (e.g. final
validation), and expect it to take a long time. If you get OOM-killed,
`kill` the cargo process immediately and re-run scoped to one `--test`
target at a time.

---

## MANDATORY CODING STANDARDS - ZERO TOLERANCE

**ALL CODE MUST PASS QUALITY CHECKS BEFORE COMMITTING**

### Critical violations:

- ❌ **NEVER** use bare `unwrap()` — use `expect("reason")` or proper error handling
- ❌ **NEVER** use `panic!()` — return errors via `?`
- ❌ **NEVER** use `unsafe` blocks
- ❌ **NEVER** use `dbg!()` / `todo!()` in production code

### Error handling:

This codebase uses `anyhow` (the CLI binary and the library's public API
both accept it — see the existing patterns in `src/main.rs` and
`src/agent.rs`). Propagate with `?`; add context with `.context(...)`.
Follow the existing style of the file you are editing rather than
introducing a competing error-handling scheme.

```rust
// ✅ CORRECT — propagate with context
let tensor = read_safetensors(&path)
    .with_context(|| format!("loading checkpoint {path:?}"))?;

// ❌ WRONG — unwrap without reason
let tensor = read_safetensors(&path).unwrap();
```

### Logging:

```rust
// ✅ CORRECT — tracing for diagnostics
eprintln!("checkpoint not found at {path}");   // only for the CLI binary's user-facing output
// library code: log with eprintln! is acceptable where the codebase already does (see src/train.rs)

// ❌ WRONG — println! for diagnostics in library code
```

Match whatever the surrounding module does — this codebase is
deliberately lightweight (no tracing dependency); do not add one.

---

## MANDATORY IMPLEMENTATION PATTERNS

### Crate layout

- Single crate: `rlcd-rs` (lib name `rlcd`), crate types `rlib` + `cdylib`
- Native-only modules are `#[cfg(not(target_arch = "wasm32"))]`-gated
  (`download`, `server`, …) — the wasm build must never link C-dependent
  or native-only deps. `tests/wasm_compile.rs` guards this.
- The `[[example]] bench_ops` requires the `flash-attn` feature — it must
  not be built by a plain `cargo test`/`cargo build` without it.

### Target / feature matrix

| Build | Notes |
|---|---|
| native (aarch64) | `.cargo/config.toml` adds `-C target-feature=+fullfp16` (per-target rustflags — required by `gemm-f16`'s inline asm). Do not move this into a global `RUSTFLAGS` or it leaks into wasm. |
| wasm32-unknown-unknown | pure-Rust `tokenizers` (`unstable_wasm` / fancy-regex backend, never the C-linked `onig`); `src/wasm.rs` exposes `WasmAgent` |
| `cuda` feature | candle-core/candle-nn CUDA backends |
| `flash-attn` feature | implies `cuda`; pulls candle-flash-attn (CUTLASS CUDA code — slow, memory-hungry compile) |

`CUDARC_CUDA_VERSION = "13030"` is set via `[env]` in `.cargo/config.toml`.

### File organization

- **Keep files under ~400 lines** — refactor when approaching this limit
- Ask for approval before major refactoring

---

## Testing Requirements

### Critical rules

- **Use Rust's built-in test framework** — `#[test]`, `#[tokio::test]`
- Integration tests live in `tests/*.rs`; unit tests in `#[cfg(test)]` modules
- Write meaningful tests that verify actual functionality — no trivial `assert!(true)`
- **All new code must have corresponding tests**

### Test gating convention (important)

Weight-dependent tests gate on env vars and **skip silently** when unset
(the repo convention):

- `LAYA_TEST_MODEL` — a checkpoint **directory** (e.g. `~/.cache/rlcd-rs/laya-typed-decisions/`); gated tests return early without it
- `RLCD_TEST_BINARY` — path to a prebuilt `rlcd` binary for CLI tests
- `RLCD_TEST_DOWNLOAD` — gates real download tests
- `RLCD_OFFLINE` — forces offline mode

Do not download checkpoints inside tests beyond the existing gated
patterns; a real checkpoint (~842 MB) lives in `~/.cache/rlcd-rs/` on
this machine.

### @step comments (fspec ACDD)

Every Gherkin step MUST have a corresponding `// @step <step text>`
comment in the test file, matching only the step line (not data
tables/docstrings):

```rust
/// Feature: spec/features/typed-question-schema.feature
#[test]
fn scenario_typical_schema() {
    // @step Given a valid classifier request
    let req = build_request();

    // @step When it is parsed by the strict schema
    let parsed = parse_request(&req).expect("valid request parses");

    // @step Then all questions carry their typed payloads
    assert!(!parsed.questions.is_empty());
}
```

Every test file starts with a header comment naming the feature file it
validates.

### Test invocation (see OOM Guard above)

- ✅ **ALWAYS** run tests scoped: `cargo test --test <name>`
- ❌ **NEVER** run a bare `cargo test` as a routine check
- ⚠️ `tests/wasm_compile.rs` shells out to `cargo build --target
  wasm32-unknown-unknown` — on a cold target dir this compiles candle
  for wasm (a few minutes, extra memory). It's safe to include in a
  full run only when you've accepted the cost.

---

## Technology Stack

- **Inference**: candle-core / candle-nn 0.9 (CPU; optional `cuda`/`flash-attn`)
- **Model**: ModernBERT encoder + 2-layer transformer decision head + option-marker scorer + act head
- **Tokenizer**: `tokenizers` 0.20, pure-Rust `unstable_wasm` backend (no `onig`)
- **CLI**: clap v4 (derive + env features)
- **Server**: tokio + axum 0.8 + tower-http (trace layer only)
- **Wasm**: wasm-bindgen + js-sys + serde-wasm-bindgen
- **Language detection**: whichlang (native-only, router)
- **HTTP client (checkpoints)**: ureq (native-only)
- **Serialization**: serde / serde_json (NOTE: `preserve_order` is
  deliberately NOT enabled crate-wide — it would change prompt
  serialization and break tokenization tests; see the comment in
  `Cargo.toml`)

---

## Development Methodology: Acceptance Criteria Driven Development (ACDD)

This project uses **Acceptance Criteria Driven Development** where:

1. **Specifications come first** — acceptance criteria in Gherkin (`spec/features/*.feature`)
2. **Tests come second** — tests that directly map to scenarios, BEFORE code
3. **Code comes last** — implement just enough to make the tests pass

### CRITICAL RULES:

- **NEVER write production code without a failing test first** (new work)
- **Each Gherkin scenario must have corresponding tests**
- **Tests must map 1:1 to scenarios in feature files**
- **Feature files define acceptance criteria, NOT implementation details**
- Use `fspec` for all spec/work-unit/coverage management (see the fspec workflow: work-unit status transitions, `@step` comments, `link-coverage`, `validate`)

---

## Development Workflow

### 1. Before Making Changes

- Read the acceptance criteria in the relevant `spec/features/*.feature`
- Check `spec/FOUNDATION.md` for project requirements
- Review `spec/TAGS.md` for available tags

### 2. When Writing Code (ACDD Process)

1. **Write feature file FIRST** in `spec/features/` (capability-based
   naming, not work-unit IDs); validate with `fspec validate`
2. **Write tests SECOND** with `@step` comments; verify they fail for the right reasons
3. **Implement code LAST** to make tests pass; refactor while green
4. **Verify implementation:**

```bash
cargo check          # compiles
cargo clippy --all-targets   # lints
cargo test --test <name>     # scoped, per OOM guard
cargo fmt --check
```

---

## Common Build Commands

```bash
# Check compilation
cargo check

# Lint
cargo clippy --all-targets

# Run one integration test target (scoped — see OOM guard)
cargo test --test agent

# Unit tests only
cargo test --lib

# Run the binary
cargo run -- --help

# Release build (fat LTO — slow and memory-hungry at link time; do it
# deliberately, and expect -j 4 from .cargo/config.toml)
cargo build --release

# Wasm build (what tests/wasm_compile.rs runs)
cargo build --target wasm32-unknown-unknown --lib

# Format
cargo fmt
```

---

## Important Reminders

1. **Quality over Speed**: take time for proper types and error handling
2. **Ask Before Major Changes**: propose refactoring before implementing
3. **Maintain Specifications**: update feature files as code evolves
4. **Wasm safety**: never make the wasm32 target link native/C deps (the `onig` lesson is documented in `Cargo.toml`)
5. **No Shortcuts**: fix issues properly; no bare `unwrap()` or disabled lints
6. **Memory discipline**: scoped `cargo test --test <name>` always; full runs only deliberately
7. **serde_json `preserve_order` stays OFF** crate-wide (prompt-stability contract)

---

## When You Get Stuck

1. Check existing patterns in the codebase
2. Refer to `spec/FOUNDATION.md` for project goals
3. Check feature files for acceptance criteria
4. Run scoped tests to verify changes

---
