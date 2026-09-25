/**
 * Feature: spec/features/cli-ask-single-question.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * The `rlcd` binary is located via the RLCD_TEST_BINARY env var (cargo sets
 * CARGO_BIN_EXE_rlcd for integration tests). Scenarios that need a real
 * checkpoint gate on LAYA_TEST_MODEL (checkpoint directory); without it they
 * are skipped — model weights are not bundled in the repository.
 */

use std::process::Command;

fn rlcd_bin() -> String {
    std::env::var("RLCD_TEST_BINARY")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_rlcd").to_string())
}

fn model_dir() -> Option<String> {
    std::env::var("LAYA_TEST_MODEL").ok()
}

/// Scenario: Missing required flags produce a usage error
#[test]
fn ask_missing_required_flags_produces_usage_error() {
    // @step Given the `rlcd` binary is available
    let bin = rlcd_bin();

    // @step When the user runs `rlcd ask` without --state or --question
    let out = Command::new(&bin)
        .args(["ask"])
        .output()
        .expect("failed to run rlcd");

    // @step Then clap rejects the command with a non-zero exit and a usage message naming the missing --state and --question flags
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--state"), "stderr should name --state: {stderr}");
    assert!(stderr.contains("--question"), "stderr should name --question: {stderr}");
}

/// Scenario: Ask a routed choice question and print probabilities
#[test]
fn ask_routed_choice_question_prints_probabilities() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    // @step Given a laya checkpoint is available and the state is "We were billed twice for March. Please refund the duplicate."
    let state = "We were billed twice for March. Please refund the duplicate.";

    // @step When the user runs `rlcd ask --state "<state>" --question "Which team should handle this?" --option billing --option technical`
    let out = Command::new(rlcd_bin())
        .args([
            "ask",
            "--model",
            &model,
            "--state",
            state,
            "--question",
            "Which team should handle this?",
            "--option",
            "billing",
            "--option",
            "technical",
        ])
        .output()
        .expect("failed to run rlcd");

    // @step Then stderr shows the routed checkpoint and resolved directory and stdout shows `choice=<option> confidence=<c> act_p=<p>` followed by one indented probability line for each option
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("routed to:"), "stderr should report the route: {stderr}");
    let first = stdout.lines().next().expect("no output");
    assert!(first.starts_with("choice="), "first line should be choice=...: {first}");
    assert!(first.contains("confidence=") && first.contains("act_p="), "first line: {first}");
    let prob_lines: Vec<&str> = stdout.lines().skip(1).collect();
    assert!(prob_lines.len() >= 2, "expected one indented line per option: {stdout}");
}

/// Scenario: Explicit --model bypasses routing and resolution
#[test]
fn explicit_model_bypasses_routing_and_resolution() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    // @step Given a checkpoint directory at /path/to/laya-typed-decisions exists and contains the five required files
    let _ = &model; // the LAYA_TEST_MODEL directory stands in for the checkpoint

    // @step When the user runs `rlcd ask --model /path/to/laya-typed-decisions --state "..." --question "..." --option a --option b`
    let out = Command::new(rlcd_bin())
        .args([
            "ask",
            "--model",
            &model,
            "--state",
            "We were billed twice.",
            "--question",
            "Which team should handle this?",
            "--option",
            "a",
            "--option",
            "b",
        ])
        .output()
        .expect("failed to run rlcd");

    // @step Then the command loads /path/to/laya-typed-decisions as-is without running the language router or checking a models root
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(&model),
        "stderr should report the explicit directory as-is: {stderr}"
    );
}
