@done
@jev-server
@decision-engine
@JEV-007
Feature: Jev protocol conformance test suite (mirroring the reference tests)
  """
  tests/jev_conformance.rs (integration) + tests/jev_conformance/*.json fixtures — HTTP-level conformance suite pinning rlcd-rs to the Jev/Simple-Jev protocol, mirroring simple-jev test_laya.py (contract) + test_api.py (validation). Two tiers: (1) weight-free — axum::Router::oneshot against build_router with a mock Answerer (no checkpoint, no GPU): validation matrix, alias, health, openapi shape, 429 queue with a blocking mock, 500 recovery; (2) model-gated (LAYA_TEST_MODEL env var, the repo's established convention) — start_server with a real RLAgent on an ephemeral port + tokio/hyper client: mixed choice/score/noul round-trip, wrong model 422, context overflow 422 (model never called), chat history, media/tool rejection, raw_logits rejection, numerical sanity (choice probs sum ~1, confidence = max, score in [0,N-1], noul in [0,1]) and determinism (identical request twice -> identical answers). Fixtures reuse the README refund example. Pass criteria: all weight-free scenarios pass on plain `cargo test`; model-gated scenarios pass under LAYA_TEST_MODEL and are skipped (not failed) otherwise; cargo clippy --all-targets clean.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # EXAMPLES:
  #   1. With LAYA_TEST_MODEL pointing at a real checkpoint, the conformance suite asserts: the mixed choice/score/noul round-trip returns protocol-shaped answers (no act_probability, noul without confidence), the wrong-model request 422s, an over-long state 422s without invoking the model, and two identical requests return byte-identical answers (determinism)
  #   2. The suite runs `cargo test` with no checkpoint and no GPU: the weight-free scenarios (validation matrix, alias, health, openapi shape, 429 with a blocking mock, 500 recovery) all pass, and the LAYA_TEST_MODEL-gated scenarios are skipped (not failed) when the env var is unset
  #
  # ========================================
  Background: User Story
    As a server implementer
    I want to prove with an automated suite that rlcd-rs stays protocol-compliant as it evolves
    So that regressions in validation, mapping, or queueing are caught before release, mirroring the reference implementation's own tests

  Scenario: The weight-free tier passes with no checkpoint and no GPU
    Given the server router is built with a mock answerer
    When the suite runs the validation matrix, alias, health, openapi shape, 429-queue (blocking mock), and 500-recovery scenarios
    Then all of them pass on a plain `cargo test` with no checkpoint, GPU, or LAYA_TEST_MODEL set

  Scenario: Model-gated scenarios skip cleanly without a checkpoint
    Given LAYA_TEST_MODEL is unset
    When the suite runs the real round-trip scenarios (mixed choice/score/noul, determinism, numerical sanity)
    Then they are skipped (not failed) and the overall suite still passes

  Scenario: Model-gated contract round-trips assert the protocol shape
    Given LAYA_TEST_MODEL points at a real laya checkpoint
    When the suite POSTs the mixed choice/score/noul fixture, a wrong-model request, and an over-long state
    Then the mixed request returns protocol-shaped answers (no act_probability, noul without confidence, usage.output_tokens 0), the wrong-model request 422s, the over-long state 422s without invoking the model, and two identical requests return byte-identical answers

  Scenario: The suite pins the validation matrix
    Given the weight-free router with a mock answerer
    When the suite exercises context XOR, unknown top-level tolerance, unknown question fields, the type discriminator, criteria cardinality (1/2/50/51), noul key restriction, empty question id, state type checks, malformed JSON, oversized body, wrong content-type, a full queue (429 + Retry-After), and a forward error (500 then recovery)
    Then each case returns the protocol status and error envelope defined in the protocol reference, and the server stays up after the 500

  Scenario: Numerical sanity and determinism hold on a real checkpoint
    Given LAYA_TEST_MODEL points at a real laya checkpoint
    When the suite answers a choice, a score, and a noul question
    Then choice probabilities sum within 0.01 of 1 with confidence equal to the max probability, the score lies in [0, N-1], the noul lies in [0, 1], and a repeated identical request returns identical answers
