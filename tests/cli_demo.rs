/**
 * Feature: spec/features/cli-demo-run.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * The `laya` binary is located via CARGO_BIN_EXE_laya (cargo sets it for
 * integration tests) or LAYA_TEST_BINARY. Both scenarios need a real
 * checkpoint (LAYA_TEST_MODEL env var naming a checkpoint directory); without
 * it they are skipped — model weights are not bundled in the repository. The
 * demo path has no `--model` flag; it resolves via `--models-root` (a
 * family-root layout with a `typed-decisions/` subfolder), so the tests build
 * that layout in a temp dir from LAYA_TEST_MODEL.
 */

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn laya_bin() -> String {
    std::env::var("LAYA_TEST_BINARY")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_laya").to_string())
}

fn model_dir() -> Option<String> {
    std::env::var("LAYA_TEST_MODEL").ok()
}

/// A temp family-root layout: `<root>/typed-decisions/` is a copy of the
/// LAYA_TEST_MODEL checkpoint directory, plus a scratch area for other files.
struct FamilyRoot {
    root: String,
    _scratch: String,
}

impl FamilyRoot {
    fn new(checkpoint: &str) -> Self {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("laya-demo-test-{n}"));
        let variant = root.join("typed-decisions");
        std::fs::create_dir_all(&variant).expect("create family root");
        for entry in std::fs::read_dir(checkpoint).expect("read checkpoint dir") {
            let entry = entry.expect("dir entry");
            let from = entry.path();
            let to = variant.join(entry.file_name());
            if from.is_dir() {
                copy_dir_recursive(&from, &to);
            } else {
                std::fs::copy(&from, &to).expect("copy checkpoint file");
            }
        }
        Self {
            root: root.to_string_lossy().to_string(),
            _scratch: n.to_string(),
        }
    }
}

fn copy_dir_recursive(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("create dir");
    for entry in std::fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("dir entry");
        let f = entry.path();
        let t = to.join(entry.file_name());
        if f.is_dir() {
            copy_dir_recursive(&f, &t);
        } else {
            std::fs::copy(&f, &t).expect("copy file");
        }
    }
}

impl Drop for FamilyRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Scenario: Demo battery prints one line per question type
#[test]
fn demo_battery_prints_one_line_per_question_type() {
    let checkpoint = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
    let family = FamilyRoot::new(&checkpoint);

    // @step Given a laya checkpoint is available
    let _ = &family.root;

    // @step When the user runs `laya "The server has been down since Tuesday and no one has replied"`
    let out = Command::new(laya_bin())
        .args(["--models-root", &family.root, "The server has been down since Tuesday and no one has replied"])
        .output()
        .expect("failed to run laya");

    // @step Then stdout shows `routed to: <checkpoint> (<dir>)` followed by a `department:` choice line with indented option probabilities, an `urgency:` score line with indented level probabilities, and a `churn_risk:` noul line
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("routed to:"), "expected the routing line: {stdout}");
    let department = stdout
        .lines()
        .find(|l| l.starts_with("department:"))
        .expect("department line");
    assert!(department.contains("choice=") && department.contains("confidence=") && department.contains("act_p="), "{department}");
    assert!(stdout.lines().any(|l| l.starts_with("    billing:") || l.starts_with("    technical:") || l.starts_with("    sales:")), "indented option probabilities expected: {stdout}");
    let urgency = stdout
        .lines()
        .find(|l| l.starts_with("urgency:"))
        .expect("urgency line");
    assert!(urgency.contains("score=") && urgency.contains("confidence="), "{urgency}");
    let churn = stdout
        .lines()
        .find(|l| l.starts_with("churn_risk:"))
        .expect("churn_risk line");
    assert!(churn.contains("noul=") && churn.contains("act_p="), "{churn}");
}

/// Scenario: No arguments use the built-in default body
#[test]
fn no_arguments_use_the_builtin_default_body() {
    let checkpoint = match model_dir() {
        Some(dir) => dir,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
    let family = FamilyRoot::new(&checkpoint);

    // @step Given a laya checkpoint is available
    let _ = &family.root;

    // @step When the user runs `laya` with no arguments at all
    let out = Command::new(laya_bin())
        .args(["--models-root", &family.root])
        .output()
        .expect("failed to run laya");

    // @step Then the demo runs against the built-in default body and prints the same three answer lines (`department:`, `urgency:`, `churn_risk:`)
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("routed to:"), "expected the routing line: {stdout}");
    assert!(stdout.lines().any(|l| l.starts_with("department:")), "department line: {stdout}");
    assert!(stdout.lines().any(|l| l.starts_with("urgency:")), "urgency line: {stdout}");
    assert!(stdout.lines().any(|l| l.starts_with("churn_risk:")), "churn_risk line: {stdout}");
}
