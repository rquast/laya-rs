@done
@decision-engine
@jev-server
@JEV-002
Feature: Jev protocol HTTP server: architecture and protocol reference
  """
  Architecture reference: docs/jev-server/architecture.md (module layout, request pipeline diagram, concurrency model, error model, testing strategy, child-card table). Protocol reference: docs/jev-server/protocol-reference.md (JSON Schema for the v1 request, response/usage/error contracts, laya-native answer semantics, CLI parity table). Both attached to this card.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The HTTP stack follows a proven axum serving pattern: axum 0.8 Router + tokio runtime, tower-http TraceLayer (CORS optional), build_router(state) factory, Clone-over-Arc state newtype injected via the axum State extractor, and a start_server -> ServerHandle { port, oneshot shutdown, JoinHandle } lifecycle with graceful shutdown
  #   2. The server implements the open Simple-Jev v1 classifier contract (POST /v1/classifier + exact alias POST /v1/systemone, GET /health) over a convaiinnovations/laya checkpoint; TypeSafe's hosted REST envelope and its five preset endpoints are explicitly out of scope
  #   3. All new HTTP server dependencies (axum, tokio, tower-http) are native-only, target-gated exactly like the existing ureq/whichlang deps, so the wasm32-unknown-unknown build of the crate is untouched; the protocol layer (request/response modules) never imports candle types — it only touches RLAgent at the system_one call boundary, keeping it weight-free testable
  #   4. Implementation is decomposed into the children JEV-003 (request schema) → JEV-005 (mapping/execution) → JEV-004 (endpoints) → JEV-006 (CLI) → JEV-007 (conformance gate); each child carries its own feature file and card doc under docs/jev-server/
  #
  # EXAMPLES:
  #   1. A simple-jev Python client pointed at the laya-rs server (base URL swapped) completes its quickstart: GET /v1/models-style discovery via /health, then POST /v1/classifier with the red-bicycle example returns a choice answer with probabilities and a zero output-token usage block
  #
  # ========================================
  Background: User Story
    As a developer or ops engineer self-hosting typed decision serving
    I want to run a Rust HTTP server speaking the open Jev/Simple-Jev classifier protocol, backed by laya-rs native inference instead of the Python/PyTorch simple-jev Laya backend
    So that any Jev-protocol client can use my self-hosted, sub-35ms laya checkpoint without a Python runtime

  Scenario: A Jev-protocol client completes the quickstart against the laya server
    Given the server is running with a loaded laya checkpoint and its /health endpoint reporting the model name
    When a simple-jev client sends GET /health then POST /v1/classifier with the red-bicycle choice example
    Then /health returns {"status":"ready","model":"<model>"} and the classifier returns a choice answer with probabilities and a usage block whose output_tokens is 0

  Scenario: The router exposes the protocol surface on a single axum router
    Given the server is built with the build_router factory and the shared Clone state
    When the client requests POST /v1/classifier, POST /v1/systemone, GET /health, and GET /openapi.json
    Then the classifier and its alias share one handler, health reports readiness without inference, and openapi.json serves the classifier contract

  Scenario: The HTTP stack follows a proven axum serving pattern
    Given the server uses axum 0.8 Router, a tokio runtime, and a tower-http TraceLayer
    When the server starts via start_server and is stopped via its handle
    Then the state is injected through the axum State extractor, shutdown is graceful (in-flight forwards complete), and the new HTTP dependencies are native-only so the wasm build is untouched

  Scenario: The protocol layer stays weight-free testable
    Given the request and response modules only touch RLAgent at the system_one boundary
    When the conformance suite runs validation, mapping, and queue scenarios with a mock answerer
    Then no checkpoint or GPU is required and the scenarios assert the protocol contract end-to-end

  Scenario: The epic decomposes into ordered child stories
    Given the parent architecture card
    When the implementation proceeds through JEV-003 (request schema), JEV-005 (mapping/execution), JEV-004 (endpoints), JEV-006 (CLI), and JEV-007 (conformance gate)
    Then each child carries its own feature file and a card doc under docs/jev-server/, and the conformance suite gates the epic to done
