@done
@decision-inference
@decision-engine
@INFER-002
Feature: Answer a batch of typed questions via CLI
  """
  The `laya answer` subcommand (src/main.rs Command::Answer) parses the input JSON ({state, questions}), resolves the checkpoint via `model_path::resolve` (the --model-variant key or an explicit --model), loads the RLAgent, and runs one `system_one` call over the (optionally --only-filtered) questions. Answers are rendered by `agent::answer_to_json` (src/agent.rs) and written as a sorted pretty-JSON {qid: answer} object. `raw_question_to_question` (src/batching.rs) maps the JSON question shapes to the typed Question.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The input file must be a JSON object with a `state` (string or object) and a `questions` object mapping question ids to {type, instructions, criteria}; missing either is an error
  #   2. Question ids map to one of three shapes: choice (criteria is an object of key→optional description or an array of strings), score (criteria is an ordered array of level descriptions), or noul (criteria is {"true": ..., "false": ...}); an empty or unknown type is a parse error
  #   3. `--only QID` restricts the batch to that single question; if the restriction leaves zero questions the command errors with `no questions to answer`
  #   4. All questions are answered in one `system_one` call; stderr reports `<n> questions in <ms> ms (<ms/question> ms/question)`, and the output file is written as pretty JSON `{qid: {type, ...}}` (choice: choice/confidence/act_probability/probabilities; score: score/legend/probabilities; noul: noul/act_probability)
  #
  # EXAMPLES:
  #   1. A user runs `laya answer input.json answers.json --model /path/to/laya-typed-decisions` where input.json holds a state and three questions (a choice, a score, a noul); the command writes answers.json with one typed answer per question id and prints a per-question timing summary to stderr
  #   2. Running `laya answer` on an input file with no `state` field fails with a `missing state` error
  #   3. Running `laya answer input.json answers.json --only churn_risk` answers only the question with id `churn_risk` and writes a single-entry answers.json
  #
  # ========================================
  Background: User Story
    As a user running the laya CLI
    I want to answer a batch of typed questions against one state in a single forward pass
    So that get typed answers for a whole question battery without writing a Rust program

  Scenario: Answer a mixed batch and write typed answers
    Given an input file contains a state and three questions: a choice with options billing/technical/sales, a score with three levels, and a noul
    When the user runs `laya answer input.json answers.json --model /path/to/laya-typed-decisions`
    Then answers.json is written as pretty JSON with one entry per question id — the choice entry has choice/confidence/act_probability/probabilities, the score entry has score/legend/probabilities, and the noul entry has noul/act_probability

  Scenario: Missing state field is an error
    Given an input file contains questions but no `state` field
    When the user runs `laya answer input.json answers.json`
    Then the command fails with a `missing state` error and no answers file is written

  Scenario: --only restricts the batch to one question
    Given an input file contains a state and several questions including one with id `churn_risk`
    When the user runs `laya answer input.json answers.json --model /path/to/laya-typed-decisions --only churn_risk`
    Then answers.json contains exactly one entry, for question id `churn_risk`

  Scenario: Malformed input file is a parse error
    Given an input file contains invalid JSON (not an object)
    When the user runs `laya answer bad.json answers.json --model /path/to/laya-typed-decisions`
    Then the command fails before any checkpoint is loaded, with a JSON parse error, and no answers file is written
