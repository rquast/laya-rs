/**
 * Feature: spec/features/cli-train-rlcd.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * `rlcd train` needs a real checkpoint directory (rl_agent_config.json,
 * tokenizer/, encoder/, model.safetensors) because `Trainer::load` runs before
 * the dataset is read. Scenarios that train gate on LAYA_TEST_MODEL (checkpoint
 * directory); without it they are skipped — model weights are not bundled in the
 * repository.
 */

use std::io::Write;
use std::process::Command;

fn rlcd_bin() -> String {
    std::env::var("RLCD_TEST_BINARY")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_rlcd").to_string())
}

fn model_dir() -> Option<String> {
    std::env::var("LAYA_TEST_MODEL").ok()
}

/// One valid plain-record line: state + one labeled choice question.
fn record_line(state: &str, label: usize) -> String {
    format!(
        r#"{{"state": "{state}", "qs": [{{"type": "choice", "instructions": "Which team should handle this?", "criteria": ["billing", "technical"], "y": {label}}}]}}"#
    )
}

fn write_jsonl(path: &std::path::Path, lines: &[String]) {
    let mut f = std::fs::File::create(path).expect("create dataset");
    for line in lines {
        f.write_all(line.as_bytes()).expect("write line");
        f.write_all(b"\n").expect("write newline");
    }
}

/// Scenario: One epoch trains and saves default-path weights
#[test]
fn one_epoch_trains_and_saves_default_path_weights() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    // @step Given a checkpoint directory and a one-line JSONL dataset
    let tmp = std::env::temp_dir().join(format!("rlcd-train-test-{}", std::process::id()));
    let dataset = tmp.join("train.jsonl");
    write_jsonl(&dataset, &[record_line("We were billed twice for March.", 0)]);

    // @step When I run `rlcd train` with default epochs
    let out = Command::new(rlcd_bin())
        .args(["train", &model, dataset.to_str().unwrap()])
        .output()
        .expect("failed to run rlcd");

    // @step Then one epoch summary line is printed
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let epoch_lines: Vec<&str> = stdout.lines().filter(|l| l.starts_with("epoch ")).collect();
    assert_eq!(epoch_lines.len(), 1, "expected one epoch line: {stdout}");
    assert!(epoch_lines[0].contains("loss=") && epoch_lines[0].contains("reward="), "epoch line: {}", epoch_lines[0]);

    // @step And the trained weights are saved to `<model_dir>/model.trained.safetensors`
    let saved = std::path::Path::new(&model).join("model.trained.safetensors");
    assert!(saved.exists(), "expected trained weights at {}", saved.display());
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Scenario: Multiple epochs log one line per pass
#[test]
fn multiple_epochs_log_one_line_per_pass() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    // @step Given a JSONL dataset with two or more records
    let tmp = std::env::temp_dir().join(format!("rlcd-train-test-epochs-{}", std::process::id()));
    let dataset = tmp.join("train.jsonl");
    write_jsonl(
        &dataset,
        &[
            record_line("We were billed twice for March.", 0),
            record_line("The login page returns 500 on every attempt.", 1),
        ],
    );

    // @step When I run `rlcd train` with `--epochs 3`
    let out = Command::new(rlcd_bin())
        .args(["train", &model, dataset.to_str().unwrap(), "--epochs", "3"])
        .output()
        .expect("failed to run rlcd");

    // @step Then three epoch summary lines are printed, one per shuffled pass
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let epoch_lines: Vec<&str> = stdout.lines().filter(|l| l.starts_with("epoch ")).collect();
    assert_eq!(epoch_lines.len(), 3, "expected three epoch lines: {stdout}");
    assert!(epoch_lines.iter().all(|l| l.contains("loss=") && l.contains("reward=")), "epoch lines: {epoch_lines:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Scenario: A malformed JSONL line aborts with a parse error
#[test]
fn a_malformed_jsonl_line_aborts_with_a_parse_error() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    // @step Given a JSONL dataset containing one line that is not a valid record
    let tmp = std::env::temp_dir().join(format!("rlcd-train-test-bad-{}", std::process::id()));
    let dataset = tmp.join("train.jsonl");
    write_jsonl(&dataset, &["not a json record at all".to_string()]);

    // @step When I run `rlcd train` over it
    let out = Command::new(rlcd_bin())
        .args(["train", &model, dataset.to_str().unwrap()])
        .output()
        .expect("failed to run rlcd");

    // @step Then the run aborts with a parse error instead of silently skipping the line
    assert!(!out.status.success(), "expected failure, stdout: {}", String::from_utf8_lossy(&out.stdout));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("parse") || stderr.to_lowercase().contains("json"),
        "stderr should report a parse/JSON error: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
