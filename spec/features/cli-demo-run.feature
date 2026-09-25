@done
@decision-inference
@decision-engine
@INFER-003
Feature: Built-in demo run against a state body
  """
  The no-subcommand path (src/main.rs lines 165-237) builds a demo state JSON {subject, body}, routes the body, resolves the checkpoint, loads the RLAgent, and answers a hard-coded 3-question battery (department choice / urgency score / churn_risk noul). Output is human-readable lines on stdout, one per question, with indented per-option probabilities. Requires a checkpoint (LAYA_TEST_MODEL env var for tests); checkpoint weights are not bundled in the repo.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. With no subcommand, `rlcd` takes a positional state body (defaulting to "We were billed twice for March. Please refund the duplicate.") and builds a demo state JSON {subject: "Duplicate charge on invoice 4411", body: <state>}
  #   2. The demo battery is exactly three questions: `department` (choice: billing/technical/sales with criteria descriptions), `urgency` (score: not urgent/soon/blocking), `churn_risk` (noul)
  #   3. The body is routed to a checkpoint, printed to stderr as `routed to: <checkpoint> (<dir>)`, and each answer prints one line: choice → `department: choice=<k> confidence=<c> act_p=<p>` + indented option probabilities; score → `urgency: score=<s> confidence=<c> act_p=<p>` + indented level probabilities; noul → `churn_risk: noul=<v> act_p=<p>`
  #
  # EXAMPLES:
  #   1. A user runs `rlcd "The server has been down since Tuesday and no one has replied"` and sees the routed checkpoint on stderr plus three answer lines — a department choice with option probabilities, an urgency score with level probabilities, and a churn_risk noul value
  #   2. A user runs `rlcd` with no arguments at all and the demo runs against the built-in default state body "We were billed twice for March. Please refund the duplicate."
  #
  # ========================================
  Background: User Story
    As a user who just installed rlcd
    I want to run `rlcd` with no subcommand to see the decision engine answer a demo question battery against a body of text
    So that a quick end-to-end smoke test of the installed binary

  Scenario: Demo battery prints one line per question type
    Given a laya checkpoint is available
    When the user runs `rlcd "The server has been down since Tuesday and no one has replied"`
    Then stdout shows `routed to: <checkpoint> (<dir>)` followed by a `department:` choice line with indented option probabilities, an `urgency:` score line with indented level probabilities, and a `churn_risk:` noul line

  Scenario: No arguments use the built-in default body
    Given a laya checkpoint is available
    When the user runs `rlcd` with no arguments at all
    Then the demo runs against the built-in default body and prints the same three answer lines (`department:`, `urgency:`, `churn_risk:`)
