@done
@decision-engine
@jev-server
@JEV-003
Feature: Strict Jev classifier request schema (parse + validate)
  """
  src/server/request.rs is a pure, weight-free module: parse_classifier_request(bytes) -> Result<ClassifierRequest, Vec<RequestError>>. No model, no IO. ClassifierRequest { model, context: State(Value)|Messages(Vec<ChatMessage>), questions: Vec<(String, rlcd::Question)> } preserves request insertion order (choice label assignment + tie-break). Reuses rlcd::Question/QType; strictness deltas over batching::RawQuestion: unknown question fields rejected, unknown noul criterion keys rejected, criteria cardinality 2-50 enforced, non-string instructions/criteria ported through the protocol canonical() (compact JSON, object keys sorted) rather than serde_json to_string. RequestError { param (dotted path with [i] indices), message, type } feeds the JEV-004 422 envelope. Context XOR is a post-parse cross-field check (empty string/{}/[] count as supplied state).
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. Unknown top-level request fields (stream, temperature, max_tokens, ...) are ignored and never cause 422; unknown question fields, unknown option fields, unknown noul criterion keys, unknown message fields, and unknown Content-Types are rejected with 422 schema-envelope errors naming the dotted field path
  #   2. Context XOR: exactly one non-null of state (string/object/array; empty string/{}/[] are valid; bare number/bool/null rejected) or messages (nonempty array of {role: system|developer|user|assistant, content: string} with no extra fields); both set or both absent → 422 'Provide exactly one of state or messages'
  #   3. Per-type question validation: choice criteria is an object of 2-50 candidate-ID keys (insertion order preserved — it assigns labels and breaks exact ties), score criteria is an ordered array of 2-50 levels, noul criteria is optional and when present an object with only the keys 'true' and/or 'false'; instructions accept string/object/array/null; all cardinality or shape violations → 422 with param naming the question id
  #   4. Non-string instructions/criteria descriptions (JSON objects or arrays) are serialized with the protocol's canonical() deterministic JSON (compact, object keys sorted) into prompt text — byte-for-byte matching simple-jev's common/prompt_builder.py canonical()
  #
  # EXAMPLES:
  #   1. A request with state "We were billed twice for March. Please refund the duplicate.", questions {route: choice(billing/technical), urgency: score(3 levels), refund: noul} and an unknown top-level field stream: true parses cleanly — the unknown field is ignored and all three questions validate
  #   2. A request with both state and messages set (or both null) fails with 422 message 'Provide exactly one of state or messages'; state: {} (empty object) with one valid question succeeds
  #   3. A messages request with one image part array in content, or a role: 'tool' message, or a 'name' field on a message fails 422 (text-only); options: {raw_logits: true} and a nonempty tools array also fail 422, while empty tools: [] is accepted
  #   4. A choice question with 51 criteria keys fails with 422 (param naming the question id); the same question with 50 keys and a noul question whose criteria has only a 'true' key both validate
  #
  # ========================================
  Background: User Story
    As a server implementer
    I want to have a typed request validated against the Jev v1 contract before any inference runs
    So that clients get readable 422 errors with dotted field paths instead of panics or 500s

  Scenario: Valid mixed batch with unknown top-level fields parses cleanly
    Given a request body with model "convaiinnovations/laya", state "We were billed twice for March. Please refund the duplicate.", and questions route (choice: billing/technical), urgency (score: 3 levels), refund (noul) plus the unknown top-level field stream: true
    When the body is parsed against the Jev v1 classifier schema
    Then parsing succeeds with the unknown top-level field ignored
    And the request keeps model "convaiinnovations/laya", the state context, and all three questions in request order

  Scenario: Context XOR violations are rejected
    Given a request body that sets both state and messages (or both to null)
    When the body is parsed against the Jev v1 classifier schema
    Then parsing fails with a 422 error whose message is 'Provide exactly one of state or messages'
    And the same body with state: {} (empty object) and one valid question instead parses successfully

  Scenario: Non-text chat messages and reserved fields are rejected
    Given a messages request whose message content is an image part array, or whose role is 'tool', or which carries a 'name' field
    When the body is parsed against the Jev v1 classifier schema
    Then parsing fails with a 422 error identifying the offending message (text messages only)
    And a request with options: {raw_logits: true} or a nonempty tools array also fails with a 422
    And the same request with an empty tools: [] is accepted

  Scenario: Choice criteria cardinality is enforced at 2-50
    Given a choice question with 51 criteria keys
    When the body is parsed against the Jev v1 classifier schema
    Then parsing fails with a 422 error whose param names the question id
    And the same question with 50 criteria keys parses successfully
    And a noul question whose criteria has only a 'true' key also parses successfully

  Scenario: Unknown question fields and noul criterion keys are rejected
    Given a question object carrying a field other than type/instructions/criteria (e.g. "context")
    When the body is parsed against the Jev v1 classifier schema
    Then parsing fails with a 422 error whose param is the dotted path into that question
    And a noul question whose criteria contains a 'maybe' key also fails with a 422

  Scenario: State values that are not string/object/array are rejected
    Given a request body whose state is a bare number (or a boolean)
    When the body is parsed against the Jev v1 classifier schema
    Then parsing fails with a 422 error naming the state field
    And state: "" and state: [] with one valid question both parse successfully

  Scenario: Non-string instructions and criteria are canonicalized
    Given a choice question whose instructions is a JSON object {"topic": "routing"} and whose candidate description is an array
    When the body is parsed against the Jev v1 classifier schema
    Then parsing succeeds and the object/array entries are serialized with the protocol canonical() deterministic JSON (compact, object keys sorted) into the question's prompt text
