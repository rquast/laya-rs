"""
Feature: spec/features/batch-answer-a-multi-section-jev-questions-file

This test file validates the acceptance criteria defined in the feature file.
Scenarios map directly to Gherkin scenarios.

The driver under test is scripts/jev_batch.py (Python 3, stdlib only). Each
test invokes it via `python3 <repo>/scripts/jev_batch.py ...` with a stand-in
`rlcd` shell-script binary that records its argv to a log file and writes a
deterministic {qid: answer} JSON file, so per-section behavior is observable
without checkpoint weights. The binary auto-detection scenario copies the
driver into a scratch repo layout instead, since the driver resolves its
built-binary candidates relative to its own location.

Runnable as plain `python3 tests/jev_batch.py` or under pytest
(`python3 -m pytest tests/jev_batch.py` — plain assert-based functions, no
test-framework dependency is added to the repository).
"""

import atexit
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DRIVER = REPO_ROOT / "scripts" / "jev_batch.py"

# A stand-in for the real `rlcd` binary: records every invocation (argv) to
# $FAKE_BINARY_LOG, fails when $FAKE_BINARY_FAIL is set, and otherwise writes
# a deterministic {qid: answer} JSON file to the section output path.
# argv: answer <input> <output> --model-dir <dir>
FAKE_BINARY = """#!/bin/sh
echo "$@" >> "$FAKE_BINARY_LOG"
if [ -n "$FAKE_BINARY_FAIL" ]; then
  echo "simulated checkpoint load failure" >&2
  exit 1
fi
python3 -c '
import json, sys
inp, outp = sys.argv[1], sys.argv[2]
data = json.loads(open(inp).read())
answers = {}
for qid, q in data["questions"].items():
    t = q.get("type", "choice")
    if t == "noul":
        answers[qid] = {"type": "noul", "noul": 0.7, "act_probability": 0.2}
    elif t == "score":
        answers[qid] = {"type": "score", "score": 1.0, "confidence": 0.8,
                        "act_probability": 0.3, "legend": {0: "low", 1: "high"},
                        "probabilities": {0: 0.2, 1: 0.8}}
    else:
        answers[qid] = {"type": "choice", "choice": "billing", "confidence": 0.9,
                        "act_probability": 0.1,
                        "probabilities": {"billing": 0.9, "technical": 0.1}}
open(outp, "w").write(json.dumps(answers, indent=2))
' "$2" "$3"
"""


class Workspace:
    """A scratch dir holding the stand-in binary and its invocation log."""

    def __init__(self, name):
        self.root = Path(tempfile.mkdtemp(prefix=f"jev-batch-{name}-"))
        self.log = self.root / "invocations.log"
        self.fake = self.root / "fake_rlcd"
        self.fake.write_text(FAKE_BINARY)
        self.fake.chmod(0o755)
        atexit.register(shutil.rmtree, self.root, ignore_errors=True)

    def env(self, fail=False):
        env = dict(os.environ)
        env["FAKE_BINARY_LOG"] = str(self.log)
        if fail:
            env["FAKE_BINARY_FAIL"] = "1"
        return env

    def run_driver(self, input_file, output_file, *extra, fail=False):
        cmd = [sys.executable, str(DRIVER), str(input_file), str(output_file), "--binary", str(self.fake)]
        cmd += list(extra)
        return subprocess.run(cmd, capture_output=True, text=True, env=self.env(fail=fail))

    def invocations(self):
        """The argv of each `rlcd answer` call the stand-in binary received (without the leading 'answer')."""
        if not self.log.is_file():
            return []
        return [line[len("answer "):] for line in self.log.read_text().splitlines() if line.startswith("answer ")]


def two_section_questions():
    """s1: one choice question; s2: one noul question (different state)."""
    return json.dumps(
        {
            "s1": {
                "state": "We were billed twice for March. Please refund the duplicate.",
                "questions": {
                    "department": {
                        "type": "choice",
                        "instructions": "Which team should handle this?",
                        "criteria": ["billing", "technical"],
                    }
                },
            },
            "s2": {
                "state": "This is my last warning before I cancel.",
                "questions": {
                    "churn_risk": {"type": "noul", "instructions": "Does the user threaten to cancel?"}
                },
            },
        }
    )


# Scenario: Answer multiple sections and assemble the output
def test_answer_multiple_sections_and_assemble_the_output():
    ws = Workspace("assemble")
    questions = ws.root / "questions.json"
    questions.write_text(two_section_questions())
    answers = ws.root / "answers.json"

    # @step Given a questions.json file contains two sections: s1 with a state and a choice question, s2 with a different state and a noul question
    sections = json.loads(questions.read_text())
    assert set(sections) == {"s1", "s2"}

    # @step When the user runs `scripts/jev_batch.py questions.json answers.json --model-dir /path/to/laya-typed-decisions` and each section's `rlcd answer` succeeds
    out = ws.run_driver(questions, answers, "--model-dir", "/path/to/laya-typed-decisions")
    assert out.returncode == 0, f"driver failed: {out.stderr}"

    # @step Then answers.json is written once as the assembled mapping s1 to the choice answer for department and s2 to the noul answer for churn_risk, and `wrote answers.json` is printed to stdout
    assert f"wrote {answers}" in out.stdout, f"stdout: {out.stdout}"
    text = answers.read_text()
    parsed = json.loads(text)
    assert set(parsed) == {"s1", "s2"}, f"assembled sections: {sorted(parsed)}"
    assert parsed["s1"]["department"]["type"] == "choice"
    assert parsed["s2"]["churn_risk"]["type"] == "noul"
    # one `rlcd answer` call per section, input-file order; pretty, keys sorted (s1 before s2)
    assert len(ws.invocations()) == 2
    assert text.index('"s1"') < text.index('"s2"')
    assert '\n  "s1"' in text  # pretty-printed (indent 2)


# Scenario: Top-level value that is not an object is rejected
def test_top_level_value_that_is_not_an_object_is_rejected():
    ws = Workspace("not-object")
    bad = ws.root / "questions.json"
    bad.write_text("[1, 2, 3]")
    answers = ws.root / "answers.json"

    # @step Given the input file's top-level JSON value is an array, not an object
    assert json.loads(bad.read_text()) == [1, 2, 3]

    # @step When the user runs the driver with that file
    out = ws.run_driver(bad, answers)

    # @step Then the driver exits with the error `expected top-level object of {section: {state, questions}}` and no `rlcd answer` call is made for any section
    assert out.returncode != 0
    assert "expected top-level object of {section: {state, questions}}" in out.stderr, f"stderr: {out.stderr}"
    assert not answers.exists(), "answers file must not be written"
    assert ws.invocations() == [], "no section may be answered"


# Scenario: A section missing its questions key aborts the run
def test_a_section_missing_its_questions_key_aborts_the_run():
    ws = Workspace("missing-questions")
    questions = ws.root / "questions.json"
    questions.write_text(json.dumps({"s1": {"state": "hello"}}))
    answers = ws.root / "answers.json"

    # @step Given the input file contains a section s1 whose body has a state but no `questions` key
    body = json.loads(questions.read_text())["s1"]
    assert "state" in body and "questions" not in body

    # @step When the user runs the driver with that file
    out = ws.run_driver(questions, answers)

    # @step Then the driver exits with the error `s1: missing questions` (the offending section named) and no answers file is written
    assert out.returncode != 0
    assert "s1: missing questions" in out.stderr, f"stderr: {out.stderr}"
    assert not answers.exists(), "answers file must not be written"
    assert ws.invocations() == []


# Scenario: --only skips sections left with no matching questions
def test_only_skips_sections_left_with_no_matching_questions():
    ws = Workspace("only")
    questions = ws.root / "questions.json"
    questions.write_text(two_section_questions())
    answers = ws.root / "answers.json"

    # @step Given the input file has a section s1 with only a department question and a section s2 with only a churn_risk question
    sections = json.loads(questions.read_text())
    assert list(sections["s1"]["questions"]) == ["department"]
    assert list(sections["s2"]["questions"]) == ["churn_risk"]

    # @step When the user runs the driver with `--only churn_risk`
    out = ws.run_driver(questions, answers, "--only", "churn_risk")
    assert out.returncode == 0, f"driver failed: {out.stderr}"

    # @step Then s1 is skipped entirely (no `rlcd answer` call for it) and answers.json contains only the s2 entry with the churn_risk answer
    parsed = json.loads(answers.read_text())
    assert set(parsed) == {"s2"}, f"expected only s2, got {sorted(parsed)}"
    assert parsed["s2"]["churn_risk"]["type"] == "noul"
    calls = ws.invocations()
    assert len(calls) == 1, f"s1 must be skipped: {calls}"
    assert "s2.in.json" in calls[0]


# Scenario: A failing section aborts the run without writing output
def test_a_failing_section_aborts_the_run_without_writing_output():
    ws = Workspace("fail")
    questions = ws.root / "questions.json"
    questions.write_text(two_section_questions())
    answers = ws.root / "answers.json"

    # @step Given the first section's `rlcd answer` invocation exits non-zero (for example, the checkpoint cannot be loaded)
    questions  # input is valid; the stand-in binary is configured to fail

    # @step When the user runs the driver with that file
    out = ws.run_driver(questions, answers, fail=True)

    # @step Then the driver aborts (subprocess check) and no answers file is written
    assert out.returncode != 0
    assert not answers.exists(), "answers file must not be written"


# Scenario: The built rlcd binary is auto-detected before PATH
def test_the_built_rlcd_binary_is_auto_detected_before_path():
    ws = Workspace("autodetect")

    # @step Given a built rlcd binary exists under the repository's target/ (release or debug), `rlcd` is not on PATH, and no `--binary` flag is given
    # Scratch repo layout: the driver's built-binary candidates are resolved
    # relative to its own location (scripts/..), so copy it in and plant a
    # built binary under target/debug/.
    repo = Path(tempfile.mkdtemp(prefix="jev-batch-autodetect-repo-"))
    atexit.register(shutil.rmtree, repo, ignore_errors=True)
    (repo / "scripts").mkdir()
    shutil.copy(DRIVER, repo / "scripts" / "jev_batch.py")
    (repo / "target" / "debug").mkdir(parents=True)
    built = repo / "target" / "debug" / "rlcd"
    built.write_text(FAKE_BINARY)
    built.chmod(0o755)
    # A `rlcd` on PATH that would be used if auto-detection fell back to PATH.
    pathdir = repo / "pathbin"
    pathdir.mkdir()
    shim = pathdir / "rlcd"
    shim.write_text('#!/bin/sh\necho "shim-called $@" >> "%s"\n' % (ws.root / "path-shim.log"))
    shim.chmod(0o755)

    questions = repo / "questions.json"
    questions.write_text(two_section_questions())
    answers = repo / "answers.json"

    # @step When the user runs the driver from a checkout whose target/ holds a built rlcd binary
    env = ws.env()
    env["PATH"] = f"{pathdir}{os.pathsep}{env['PATH']}"
    cmd = [sys.executable, str(repo / "scripts" / "jev_batch.py"), str(questions), str(answers), "--model-dir", "/path/to/laya-typed-decisions"]
    out = subprocess.run(cmd, capture_output=True, text=True, env=env)
    assert out.returncode == 0, f"driver failed: {out.stderr}"

    # @step Then the driver invokes the built binary under target/ (the `rlcd` on PATH is never used) and the run proceeds to answer the sections with it
    assert len(ws.invocations()) == 2, "both sections must be answered by the built binary"
    assert not (ws.root / "path-shim.log").exists(), "the `rlcd` on PATH must never be used"
    assert json.loads(answers.read_text()) and set(json.loads(answers.read_text())) == {"s1", "s2"}


# Scenario: The driver forwards its checkpoint flag to every section call
def test_the_driver_forwards_its_checkpoint_flag_to_every_section_call():
    ws = Workspace("flag-forwarding")
    questions = ws.root / "questions.json"
    questions.write_text(two_section_questions())
    answers = ws.root / "answers.json"

    # @step Given a stand-in rlcd binary records the full argument list of each `rlcd answer` invocation it receives, and a valid multi-section questions file exists
    assert ws.fake.is_file() and set(json.loads(questions.read_text())) == {"s1", "s2"}

    # @step When the user runs the driver with `--model-dir /path/to/laya-typed-decisions` against that file
    out = ws.run_driver(questions, answers, "--model-dir", "/path/to/laya-typed-decisions")
    assert out.returncode == 0, f"driver failed: {out.stderr}"

    # @step Then each recorded `rlcd answer` invocation carries the `--model-dir /path/to/laya-typed-decisions` argument (the as-built flag contract — see the architecture notes for how it mismatches the current binary's --model)
    calls = ws.invocations()
    assert len(calls) == 2, f"one invocation per section: {calls}"
    for call in calls:
        assert "--model-dir /path/to/laya-typed-decisions" in call, f"flag not forwarded: {call}"


def main():
    tests = sorted((name, fn) for name, fn in list(globals().items()) if name.startswith("test_") and callable(fn))
    failed = 0
    for name, fn in tests:
        try:
            fn()
        except AssertionError as e:
            failed += 1
            print(f"FAIL {name}: {e}")
        except Exception as e:  # noqa: BLE001 - test runner must report any error
            failed += 1
            print(f"ERROR {name}: {type(e).__name__}: {e}")
        else:
            print(f"ok   {name}")
    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
