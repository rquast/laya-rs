@done
@decision-engine
@jev-server
@JEV-004
Feature: Jev HTTP endpoints: router, classifier + systemone alias, health, openapi, error envelope
  """
  src/server/router.rs + src/server/handlers/ (standard axum router-factory pattern): build_router(state) -> Router with .route("/v1/classifier", post(classify)).route("/v1/systemone", post(classify)).route("/health", get(health)).route("/openapi.json", get(openapi)).with_state(state).layer(TraceLayer::new_for_http()) + DefaultBodyLimit::max(1 MiB). ServerState is a Clone newtype over Arc<Inner> (agent, model_name, config, model_lock, admission semaphore) — the axum State extractor. handlers/error.rs owns the protocol envelope builders: 422 {error:{message,type:'invalid_request_error',code:422,param,details[]}} (up to 10 details, extras folded into the summary), 429 {"detail":"Scoring queue is full"} + Retry-After: 1, 499 {"detail":"Client disconnected"}, 500 {"detail":"internal error"}. Handler never panics: every failure path maps to a protocol status. The classifier handler takes the request body as bytes (Content-Type sniffed: missing tolerated, non-JSON 422), validates via JEV-003, checks model identity, then delegates to the JEV-005 execution seam. openapi.json is a static in-crate OpenAPI 3.1 document (no codegen dep).
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. POST /v1/classifier and POST /v1/systemone are the same handler (exact alias, identical behavior and response shape); GET /health returns {"status":"ready","model":"<loaded model>"} without running any inference; GET /openapi.json serves the static OpenAPI 3.1 document of the classifier endpoints
  #   2. Every protocol error uses the shared envelope: 422 {error: {message, type: 'invalid_request_error', code: 422, param, details[]}} with up to 10 detail entries (dotted paths, [i] array indices, extra errors folded into the summary message); 429 short form {"detail":"Scoring queue is full"} with Retry-After: 1; 499 {"detail":"Client disconnected"}; 500 {"detail":"internal error"} for unhandled runtime failures (no stable body guaranteed)
  #   3. Router-level guards: request bodies over 1 MiB and non-JSON Content-Type are rejected with 422 before JSON parsing; the classifier handler never panics on malformed input (every validation path yields a readable 422/429)
  #
  # EXAMPLES:
  #   1. Sending a mixed choice/score/noul body to POST /v1/systemone returns a byte-equivalent answers/usage shape to POST /v1/classifier; GET /health returns {"status":"ready","model":"convaiinnovations/laya"} without running a forward pass
  #   2. A request body of 2 MiB, or a valid JSON body with Content-Type: text/plain, returns the 422 envelope (no JSON parsing happens for the oversized body); a request whose model field is 'other' while the server loaded 'convaiinnovations/laya' returns 422 with message "Loaded model is 'convaiinnovations/laya'"
  #   3. When the forward pass raises an internal error, the response is 500 with body {"detail":"internal error"} and the server keeps serving subsequent requests
  #
  # ========================================
  Background: User Story
    As a protocol client developer
    I want to expose the classifier behind POST /v1/classifier (alias /v1/systemone), GET /health and GET /openapi.json on an axum router
    So that protocol clients can discover and reach the service with standard HTTP semantics and readable error envelopes

  Scenario: The systemone alias behaves identically to classifier
    Given the server is running with model "convaiinnovations/laya"
    When the client POSTs a mixed choice/score/noul body to /v1/systemone
    Then the response's answers and usage are byte-equivalent in shape to the /v1/classifier response for the same body

  Scenario: Health reports readiness without inference
    Given the server is running with model "convaiinnovations/laya"
    When the client GETs /health
    Then the response is 200 with body {"status":"ready","model":"convaiinnovations/laya"}
    And no forward pass was executed (assertable with an instrumented answerer)

  Scenario: Oversized bodies and wrong content types are rejected before parsing
    Given the server is running
    When the client POSTs a 2 MiB JSON body to /v1/classifier, or a valid JSON body with Content-Type: text/plain
    Then both requests receive the 422 error envelope and no JSON parsing or inference occurred

  Scenario: Model identity mismatch is a 422 with the loaded model named
    Given the server loaded model "convaiinnovations/laya"
    When the client POSTs an otherwise-valid body whose model field is "other"
    Then the response is 422 with error.message "Loaded model is 'convaiinnovations/laya'"

  Scenario: Internal forward errors surface as 500 and the server keeps serving
    Given the answerer is configured to raise an internal error on the next forward
    When the client POSTs a valid body to /v1/classifier
    Then the response is 500 with body {"detail":"internal error"}
    And a subsequent valid request succeeds (the server stays up)

  Scenario: The OpenAPI document is served for discovery
    Given the server is running
    When the client GETs /openapi.json
    Then the response is 200 with a static OpenAPI 3.1 document describing /v1/classifier, /v1/systemone, and /health
