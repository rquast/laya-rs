/**
 * Feature: spec/features/cli-answer-batch-questions.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * The `rlcd` binary is located via CARGO_BIN_EXE_rlcd (cargo sets it for
 * integration tests) or RLCD_TEST_BINARY. Scenarios that need a real
 * checkpoint gate on LAYA_TEST_MODEL (checkpoint directory); without it they
 * are skipped — model weights are not bundled in the repository.
 */

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn rlcd_bin() -> String {
    std::env::var("RLCD_TEST_BINARY")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_rlcd").to_string())
}

fn model_dir() -> Option<String> {
    std::env::var("LAYA_TEST_MODEL").ok()
}

/// A unique temp dir for input/output files (cleaned up on drop).
struct TmpDir(String);
impl TmpDir {
    fn new() -> Self {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let d = std::env::temp_dir().join(format!("rlcd-answer-test-{n}"));
        std::fs::create_dir_all(&d).expect("temp dir");
        Self(d.to_string_lossy().to_string())
    }
    fn join(&self, name: &str) -> String {
        format!("{}/{}", self.0, name)
    }
}
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Scenario: Answer a mixed batch and write typed answers
#[test]
fn answer_mixed_batch_writes_typed_answers() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    let tmp = TmpDir::new();
    let input = tmp.join("input.json");
    let output = tmp.join("answers.json");
    std::fs::write(
        &input,
        r#"{
          "state": "We were billed twice for March. Please refund the duplicate.",
          "questions": {
            "department": {"type": "choice", "instructions": "Which team should handle this?",
              "criteria": {"billing": "invoices, payments, refunds", "technical": "bugs and outages", "sales": "pricing"}},
            "urgency": {"type": "score", "instructions": "How urgent is this?",
              "criteria": ["not urgent", "soon", "blocking"]},
            "churn_risk": {"type": "noul", "instructions": "Does the user threaten to cancel?"}
          }
        }"#,
    )
    .expect("write input");

    // @step Given an input file contains a state and three questions: a choice with options billing/technical/sales, a score with three levels, and a noul
    let _ = &input;

    // @step When the user runs `rlcd answer input.json answers.json --model /path/to/laya-typed-decisions`
    let out = Command::new(rlcd_bin())
        .args(["answer", &input, &output, "--model", &model])
        .output()
        .expect("failed to run rlcd");

    // @step Then answers.json is written as pretty JSON with one entry per question id — the choice entry has choice/confidence/act_probability/probabilities, the score entry has score/legend/probabilities, and the noul entry has noul/act_probability
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let answers: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output).expect("read answers")).expect("parse");
    let obj = answers.as_object().expect("object");
    assert_eq!(obj.len(), 3, "one entry per question id: {answers}");
    let dept = &obj["department"];
    assert_eq!(dept["type"], "choice");
    assert!(dept.get("choice").is_some() && dept.get("confidence").is_some() && dept.get("act_probability").is_some() && dept.get("probabilities").is_some());
    let urgency = &obj["urgency"];
    assert_eq!(urgency["type"], "score");
    assert!(urgency.get("score").is_some() && urgency.get("legend").is_some() && urgency.get("probabilities").is_some());
    let churn = &obj["churn_risk"];
    assert_eq!(churn["type"], "noul");
    assert!(churn.get("noul").is_some() && churn.get("act_probability").is_some());
}

/// Scenario: Missing state field is an error
#[test]
fn missing_state_field_is_an_error() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    let tmp = TmpDir::new();
    let input = tmp.join("input.json");
    let output = tmp.join("answers.json");
    std::fs::write(&input, r#"{"questions": {"q1": {"type": "choice", "instructions": "x?", "criteria": ["a", "b"]}}}"#)
        .expect("write input");

    // @step Given an input file contains questions but no `state` field
    let _ = &input;

    // @step When the user runs `rlcd answer input.json answers.json`
    let out = Command::new(rlcd_bin())
        .args(["answer", &input, &output, "--model", &model])
        .output()
        .expect("failed to run rlcd");

    // @step Then the command fails with a `missing state` error and no answers file is written
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("missing state"), "stderr: {stderr}");
    assert!(!std::path::Path::new(&output).exists(), "answers file must not be written");
}

/// Scenario: --only restricts the batch to one question
#[test]
fn only_restricts_batch_to_one_question() {
    let model = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };

    let tmp = TmpDir::new();
    let input = tmp.join("input.json");
    let output = tmp.join("answers.json");
    std::fs::write(
        &input,
        r#"{
          "state": "We were billed twice.",
          "questions": {
            "department": {"type": "choice", "instructions": "Which team?", "criteria": ["billing", "technical"]},
            "churn_risk": {"type": "noul", "instructions": "Does the user threaten to cancel?"}
          }
        }"#,
    )
    .expect("write input");

    // @step Given an input file contains a state and several questions including one with id `churn_risk`
    let _ = &input;

    // @step When the user runs `rlcd answer input.json answers.json --model /path/to/laya-typed-decisions --only churn_risk`
    let out = Command::new(rlcd_bin())
        .args(["answer", &input, &output, "--model", &model, "--only", "churn_risk"])
        .output()
        .expect("failed to run rlcd");

    // @step Then answers.json contains exactly one entry, for question id `churn_risk`
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let answers: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&output).expect("read answers")).expect("parse");
    let obj = answers.as_object().expect("object");
    assert_eq!(obj.len(), 1, "only the restricted question: {answers}");
    assert!(obj.contains_key("churn_risk"));
}

/// Scenario: Malformed input file is a parse error
#[test]
fn malformed_input_file_is_a_parse_error() {
    let tmp = TmpDir::new();
    let input = tmp.join("bad.json");
    let output = tmp.join("answers.json");
    std::fs::write(&input, "this is { not json").expect("write bad input");

    // @step Given an input file contains invalid JSON (not an object)
    let _ = &input;

    // @step When the user runs `rlcd answer bad.json answers.json --model /path/to/laya-typed-decisions`
    let out = Command::new(rlcd_bin())
        .args(["answer", &input, &output, "--model", "/nonexistent/model-dir"])
        .output()
        .expect("failed to run rlcd");

    // @step Then the command fails before any checkpoint is loaded, with a JSON parse error, and no answers file is written
    assert!(!out.status.success());
    assert!(!std::path::Path::new(&output).exists(), "answers file must not be written");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // serde_json's parse error for `this is { not json`
    assert!(
        stderr.contains("expected ident at line 1 column 2"),
        "expected the serde_json parse error, got stderr: {stderr} stdout: {stdout}"
    );
    // the /nonexistent path must never have been loaded
    assert!(!stderr.contains("Downloading"), "must not attempt a download: {stderr}");
}
