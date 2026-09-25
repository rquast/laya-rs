/**
 * Feature: spec/features/wasm-browser-agent.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * Re-scoped to the wasm-specific criterion only: the lib compiles for
 * wasm32-unknown-unknown with the pure-Rust (`unstable_wasm`) tokenizers
 * backend and no C-linked (`onig`) regex backend in the dependency tree. The
 * load/ask business logic is shared with the native CLI path and is covered by
 * tests/schema.rs, tests/cli_answer.rs, and tests/agent.rs; running the wasm
 * in a real VM is intentionally out of scope for the test path.
 *
 * NOTE: this shells out to `cargo build --target wasm32-unknown-unknown`. On a
 * clean CI runner the first invocation compiles candle for wasm (a few
 * minutes); on a warm target dir it is a no-op. The outer `cargo test` holds
 * the build lock only while compiling the test binary, so by the time this
 * #[test] runs and spawns the nested `cargo build` the lock is free and there
 * is no deadlock.
 */

/// Scenario: The crate compiles for the browser target
#[test]
fn the_crate_compiles_for_the_browser_target() {
    // @step Given the rlcd library with its pure-Rust tokenizers backend
    // The backend is fixed in Cargo.toml: `tokenizers` with the `unstable_wasm`
    // (pure-Rust `fancy-regex`) feature, never the C-linked `onig` default.
    // Nothing to do here — this step asserts the precondition the build below
    // is compiled against.

    // @step When I compile the lib for wasm32-unknown-unknown
    let build = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--lib", "--quiet"])
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("spawn `cargo build --target wasm32-unknown-unknown --lib`");
    assert!(
        build.status.success(),
        "wasm32 lib build failed (exit {:?}):\nstdout: {}\nstderr: {}",
        build.status.code(),
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    // @step Then the build succeeds with no C-linked tokenizer backend in the dependency tree
    // `onig` is the C-linked (Oniguruma) regex backend; its presence would mean
    // the tokenizers crate fell back to a C library that cannot link for wasm32.
    let tree = std::process::Command::new("cargo")
        .args(["tree", "--target", "wasm32-unknown-unknown", "-e", "normal", "--quiet"])
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .expect("spawn `cargo tree --target wasm32-unknown-unknown`");
    let dep_tree = String::from_utf8_lossy(&tree.stdout);
    assert!(
        !dep_tree.contains("onig"),
        "a C-linked tokenizer backend (onig) leaked into the wasm32 dependency tree:\n{dep_tree}"
    );
}
