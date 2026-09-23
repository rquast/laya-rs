@done
@jev-server
@decision-engine
@JEV-005
Feature: Laya-native answer mapping, usage accounting, serial execution + 429 admission queue
  """
  src/server/response.rs + execution layer in src/server/state.rs. answer_to_jev_json(answer, question) maps laya::Answer to the protocol shape: choice {type, choice, confidence, probabilities keyed in request insertion order} (no act_probability); score {type, score=sum(p[i]*i), confidence=max level prob, probabilities keyed "0".."N-1", legend mapping those keys to criteria}; noul {type, noul} (native P(true), no confidence). Distinct from agent::answer_to_json (which BTreeMap-sorts and emits act_probability). usage.input_tokens = sum of per-question sequence lengths (re-tokenizes context, repeated context counted); output_tokens=0. Execution: model_lock (tokio::sync::Mutex) ensures one forward at a time; admission Semaphore(max_queued) bounds the waiting queue; forward runs under spawn_blocking (sync candle call). Pre-inference admission builds each sequence via schema::build_sequence and 422s on marker loss / token cap / branch-limit BEFORE calling system_one. The forward is behind an Answerer trait seam so the router/mapping are test-injectable and weight-free.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. Laya-native answer mapping: choice answers carry choice/confidence/probabilities with candidate keys in the request's insertion order; score answers carry score (expected zero-based index sum(p[i]*i))/confidence (largest level probability)/probabilities keyed "0".."N-1"/legend mapping those keys to the original criteria; noul answers carry ONLY type + noul (the native P(true), no confidence field); the laya act_probability field is stripped from every answer
  #   2. usage.input_tokens is the sum of the actual per-question sequence lengths (laya re-tokenizes context per question, so repeated context is counted — matching the reference laya backend), and usage.output_tokens is always 0
  #   3. Pre-inference admission (422 before any forward pass): each compiled question sequence must fit min(--max-model-len, checkpoint max_len) — no automatic truncation at the protocol layer; question count must not exceed --max-request-branches (default 100, schema hard cap 256). Overflow or excess is a 422 naming the question id and the cap, and the model is never called
  #   4. Serial execution: exactly one model forward at a time (tokio model lock — matching the reference 'requests execute serially against the model'); all parallelism happens within a request (one system_one call over all questions); the synchronous candle forward runs under tokio::task::spawn_blocking, never on an async runtime thread
  #   5. Admission queue: one in-flight forward + up to --max-queued (default 16) waiting requests (the reference's 1+16 model); the next request arriving at capacity gets 429 with Retry-After: 1; a client disconnect cancels its queued future, and if a response can still be delivered it is 499 (an in-flight forward cannot be interrupted — the model lock is held until it completes)
  #
  # EXAMPLES:
  #   1. A 200 response for a mixed request contains answers.route {type: choice, choice: 'billing', confidence, probabilities keyed in request order billing/technical — and no act_probability field}, answers.urgency {type: score, score: 1.75, confidence: 0.8, probabilities {"0":..,"1":..,"2":..}, legend {"0":"Routine",...}}, and answers.refund {type: noul, noul: 0.9}
  #   2. For the mixed three-question request above, usage.input_tokens equals the sum of the three per-question tokenized sequence lengths (the shared state text is counted three times) and usage.output_tokens is 0
  #   3. With --max-queued 1 and a slow in-flight forward, the third concurrent request receives 429 {"detail":"Scoring queue is full"} with header Retry-After: 1 while the first two proceed; exactly one forward is ever running at a time (verifiable with an instrumented mock answerer)
  #
  # ========================================
  Background: User Story
    As a server implementer
    I want to map laya's typed answers to the Jev protocol response shape and run the model serially with a bounded queue
    So that responses are numerically faithful to the laya backend contract and the server stays stable under concurrent load

  Scenario: A mixed request maps to laya-native protocol answers
    Given a completed mixed request with a choice (billing/technical), a score (3 levels), and a noul question
    When the answers are mapped to the Jev protocol response shape
    Then answers.route is {type: choice, choice: 'billing', confidence, probabilities keyed in request order billing/technical} with no act_probability field
    And answers.urgency is {type: score, score: 1.75, confidence: 0.8, probabilities {"0":..,"1":..,"2":..}, legend {"0":"Routine",...}}
    And answers.refund is {type: noul, noul: 0.9} with no confidence field

  Scenario: Usage accounting sums per-question sequence lengths
    Given the mixed three-question request above
    When the response usage is computed
    Then usage.input_tokens equals the sum of the three per-question tokenized sequence lengths (the shared state text is counted three times)
    And usage.output_tokens is 0

  Scenario: Oversized questions are rejected before inference
    Given a request whose one question's compiled sequence exceeds the effective max-model-len cap
    When the request is admitted
    Then the response is 422 naming the question id and the cap
    And no forward pass was executed (the model was never called)

  Scenario: Excess question count is rejected before inference
    Given a request with more questions than --max-request-branches (100)
    When the request is admitted
    Then the response is 422 naming the branch limit and no forward pass was executed

  Scenario: A full admission queue yields 429 with Retry-After
    Given the server is configured with --max-queued 1 and one slow forward is in flight
    When a third concurrent request arrives
    Then it receives 429 {"detail":"Scoring queue is full"} with header Retry-After: 1
    And the first two requests proceed and exactly one forward is ever running at a time (verified with an instrumented mock answerer)

  Scenario: Forward errors and disconnects do not corrupt the queue
    Given a forward that raises an internal error, or a client that disconnects while queued
    When the request completes (or is cancelled)
    Then the model lock is released (the in-flight forward runs to completion), subsequent requests are served, and a disconnect that can still be answered is reported as 499 {"detail":"Client disconnected"}
