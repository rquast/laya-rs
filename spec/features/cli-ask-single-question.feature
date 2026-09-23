@done
@decision-engine
@decision-inference
@INFER-001
Feature: Ask a single choice question via CLI
  """
  The `laya ask` subcommand (src/main.rs Command::Ask) builds one Choice Question from --state/--question/--option, routes the state via `route()` to a checkpoint variant, resolves the directory via `model_path::resolve` (explicit --model wins; else family root; else cached download), loads the RLAgent, calls `system_one`, and prints the answer. Checkpoint weights are not bundled with the repository.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The state is routed through `route(state)` to pick the English or multilingual checkpoint, then resolved via model_path::resolve using the routed variant key; an explicit --model/LAYA_MODEL path bypasses routing and resolution entirely
  #   2. The CLI prints the routed checkpoint and resolved directory to stderr, then prints `choice=<key> confidence=<c> act_p=<p>` followed by one indented `<option>: <prob>` line per option
  #   3. `laya ask` requires --state and --question; --option is a repeatable flag and each value becomes a choice criterion with no description
  #
  # EXAMPLES:
  #   1. Running `laya ask --state "We were billed twice" --question "Which team should handle this?" --option billing --option technical` routes the state, loads the routed checkpoint, and prints the chosen option with its probability distribution
  #   2. Running `laya ask --model /path/to/laya-typed-decisions --state "..." --question "..." --option a --option b` loads the given checkpoint directory as-is, skipping routing and models-root resolution
  #
  # ========================================
  Background: User Story
    As a user running the laya CLI
    I want to ask a single choice question against a state
    So that smoke-test a checkpoint without writing a JSON batch file

  Scenario: Ask a routed choice question and print probabilities
    Given a laya checkpoint is available and the state is "We were billed twice for March. Please refund the duplicate."
    When the user runs `laya ask --state "<state>" --question "Which team should handle this?" --option billing --option technical`
    Then stderr shows the routed checkpoint and resolved directory and stdout shows `choice=<option> confidence=<c> act_p=<p>` followed by one indented probability line for each option

  Scenario: Explicit --model bypasses routing and resolution
    Given a checkpoint directory at /path/to/laya-typed-decisions exists and contains the five required files
    When the user runs `laya ask --model /path/to/laya-typed-decisions --state "..." --question "..." --option a --option b`
    Then the command loads /path/to/laya-typed-decisions as-is without running the language router or checking a models root

  Scenario: Missing required flags produce a usage error
    Given the `laya` binary is available
    When the user runs `laya ask` without --state or --question
    Then clap rejects the command with a non-zero exit and a usage message naming the missing --state and --question flags
