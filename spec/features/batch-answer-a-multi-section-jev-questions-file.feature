@done
@decision-inference
@decision-engine
@INFER-007
Feature: Batch-answer a multi-section jev-questions file
  """
  scripts/jev_batch.py is a thin multi-section batch driver layered on the `rlcd answer` subcommand (INFER-002): the binary only understands one state plus its questions, so the script loops over a jev-questions-style {section: {state, questions}} file, writes each section to <tmp>/<section>.in.json, invokes `rlcd answer <in> <out> --model-dir <DIR>` (subprocess, check=True, stdout suppressed, stderr passed through), and assembles {section: {qid: answer}} into the output file (json.dump indent=2, sort_keys=True). Binary resolution: --binary flag, else <repo-root>/target/release/rlcd, else <repo-root>/target/debug/rlcd (repo root = Path(__file__).parent.parent), else `rlcd` on PATH; --model-dir defaults to /mnt/extra/ai/laya/typed-decisions. Per-section validation (top-level object, state+questions keys, --only filter leaving zero questions) happens before any subprocess call; a non-zero exit from `rlcd answer` propagates as CalledProcessError and no output file is written. AS-BUILT CONTRACT MISMATCH (tracked): the driver passes `--model-dir` but the current `Answer` subcommand defines `--model` (env LAYA_MODEL) / `--model-variant` / `--models-root` and rejects `--model-dir`, so the end-to-end path fails on every section until the flag contract is reconciled (the `rlcd ask` docs in README has the same stale `--model-dir` references).
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The driver calls the rlcd binary once per section, in input-file order, passing the section's state and questions as {state, questions} JSON plus the user's checkpoint flags; each section's stdout is suppressed and its stderr passes through
  #   2. The input file must be a JSON object mapping section names to {state, questions} bodies; a top-level value that is not an object is an error before any section is processed
  #   3. Each section body must contain both a `state` and a `questions` key; a section missing either is a hard error naming the section, aborting the run before its `rlcd answer` call
  #   4. `--only QID` is applied per section: questions whose id does not match are dropped before the `rlcd answer` call, and a section left with zero questions after filtering is skipped entirely — it gets no `rlcd answer` call and no entry in the output
  #   5. A section's `rlcd answer` invocation exiting non-zero (bad question shape, missing state inside, no questions after filtering, checkpoint load failure, ...) aborts the whole run via subprocess check; no output file is written
  #   6. The rlcd binary is located in order: an explicit --binary flag, else target/release/rlcd in the repository root (two directories up from scripts/), else target/debug/rlcd, else `rlcd` from PATH; the default --model-dir points at /mnt/extra/ai/laya/typed-decisions
  #   7. Each section's intermediate {state, questions} input and {qid: answer} output files live in a single temporary directory named <section>.in.json / <section>.out.json, which is removed when the run finishes (success or failure); the temp path is not exposed to the user
  #   8. The output file is written exactly once, after all sections succeed, as a JSON object mapping each section to a {qid: that section's rlcd answer result for that qid} object, pretty-printed with keys sorted at every level
  #
  # EXAMPLES:
  #   1. questions.json holds two sections (s1 with a choice question, s2 with a noul question); running the driver writes answers.json as {"s1": {"department": {...choice...}}, "s2": {"churn_risk": {...noul...}}} and prints `wrote answers.json`
  #   2. Running the driver on a file whose top-level value is a JSON array (or any non-object) exits with `expected top-level object of {section: {state, questions}}` and runs no `rlcd answer` call
  #   3. A section body missing its `questions` key exits with `s1: missing questions` (the section name in the error) and writes no output
  #   4. With `--only churn_risk` against a file where only section s2 has that question id, s1 is skipped entirely (no `rlcd answer` call, no `wrote` line for it) and answers.json contains only the s2 entry
  #   5. Pointing `--binary` at a non-writable or missing checkpoint makes the section's `rlcd answer` exit non-zero; the driver aborts (subprocess check) and writes no answers.json
  #   6. In a fresh clone the driver auto-detects target/debug/rlcd (or target/release/rlcd if present) without --binary; when neither build exists it falls back to `rlcd` on PATH
  #   7. CONTRACT MISMATCH (as-built): the driver passes `--model-dir DIR` to `rlcd answer`, but the current `Answer` subcommand (src/main.rs:66-77) accepts `--model` (env LAYA_MODEL), `--model-variant` and `--models-root` — never `--model-dir`. Running today's script against today's binary fails with `error: unexpected argument '--model-dir' found` for the first section, so no answers.json is written. README.md:159-166 also still document `--model-dir` for `rlcd ask`/`rlcd answer`
  #
  # ASSUMPTIONS:
  #   1. Reverse ACDD convention: the feature file documents as-built behavior, including the as-built `--model-dir`/`--model` contract mismatch (kept visible as a scenario), rather than the spec unilaterally fixing the driver
  #
  # ========================================
  Background: User Story
    As a user running a jev-questions battery across multiple sections
    I want to batch-answer every section through the rlcd CLI driver script
    So that get a {section: {qid: answer}} file without writing glue code

  Scenario: Answer multiple sections and assemble the output
    Given a questions.json file contains two sections: s1 with a state and a choice question, s2 with a different state and a noul question
    When the user runs `scripts/jev_batch.py questions.json answers.json --model-dir /path/to/laya-typed-decisions` and each section's `rlcd answer` succeeds
    Then answers.json is written once as the assembled mapping s1 to the choice answer for department and s2 to the noul answer for churn_risk, and `wrote answers.json` is printed to stdout

  Scenario: Top-level value that is not an object is rejected
    Given the input file's top-level JSON value is an array, not an object
    When the user runs the driver with that file
    Then the driver exits with the error `expected top-level object of {section: {state, questions}}` and no `rlcd answer` call is made for any section

  Scenario: A section missing its questions key aborts the run
    Given the input file contains a section s1 whose body has a state but no `questions` key
    When the user runs the driver with that file
    Then the driver exits with the error `s1: missing questions` (the offending section named) and no answers file is written

  Scenario: --only skips sections left with no matching questions
    Given the input file has a section s1 with only a department question and a section s2 with only a churn_risk question
    When the user runs the driver with `--only churn_risk`
    Then s1 is skipped entirely (no `rlcd answer` call for it) and answers.json contains only the s2 entry with the churn_risk answer

  Scenario: A failing section aborts the run without writing output
    Given the first section's `rlcd answer` invocation exits non-zero (for example, the checkpoint cannot be loaded)
    When the user runs the driver with that file
    Then the driver aborts (subprocess check) and no answers file is written

  Scenario: The built rlcd binary is auto-detected before PATH
    Given a built rlcd binary exists under the repository's target/ (release or debug), `rlcd` is not on PATH, and no `--binary` flag is given
    When the user runs the driver from a checkout whose target/ holds a built rlcd binary
    Then the driver invokes the built binary under target/ (the `rlcd` on PATH is never used) and the run proceeds to answer the sections with it

  Scenario: The driver forwards its checkpoint flag to every section call
    Given a stand-in rlcd binary records the full argument list of each `rlcd answer` invocation it receives, and a valid multi-section questions file exists
    When the user runs the driver with `--model-dir /path/to/laya-typed-decisions` against that file
    Then each recorded `rlcd answer` invocation carries the `--model-dir /path/to/laya-typed-decisions` argument (the as-built flag contract — see the architecture notes for how it mismatches the current binary's --model)
