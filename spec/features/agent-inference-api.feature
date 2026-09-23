@done
@decision-inference
@decision-engine
@INFER-005
Feature: Answer typed questions through the Rust library API
  """
  src/agent.rs: RLAgent {tok, special, model, cfg, device}; load() reads rl_agent_config.json + tokenizer/{tokenizer.json,tokenizer_config.json} + encoder/config.json + model.safetensors (mmaped; F16 on CUDA, F32 on CPU); load_from_bytes (98) same from bytes, always CPU/F32 via safetensors32::load_buffer (wasm path); system_one (126) builds per-question sequences (build_sequence; markers must == render_options len else bail 'options do not fit in head_max_len'), pads batch, ONE DecisionModel::forward -> (logits, act_probs); temperature: cfg.temperature_by_options[temp_bucket(qtype,k)] -> cfg.temperature[qtype_idx] -> 1.0; softmax of logits/temp; Answer::{Choice(argmax key + probs + confidence + act_probability), Score(weighted index mean, legend, probs), Noul(p[1], act_probability)}; answer_to_json (211) {"type":"choice|score|noul",...}. confidence_from_probs in src/metrics.rs (29); temp_bucket in src/batching.rs (343); render_options in src/schema.rs.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. A checkpoint loads from a directory (rl_agent_config.json, tokenizer/, encoder/config.json, model.safetensors) via RLAgent::load; load_from_bytes does the same from in-memory bytes and is always CPU/F32 (the browser has no filesystem)
  #   2. Answers are typed: choice returns the argmax option key with per-option probabilities; score returns a weighted index mean over the legend; noul returns the false-probability as a [0,1] float — each with confidence and act_probability
  #   3. system_one answers all questions in ONE encoder+decision-head forward pass: sequences are padded into a batch, option markers drive the head; logits are scaled by the per-(qtype, option-count) temperature (falling back to per-qtype, then 1.0) and softmaxed into probabilities
  #
  # EXAMPLES:
  #   1. Calling system_one with a question whose options exceed head_max_len fails with an error naming the question id and head_max_len instead of returning a wrong answer
  #   2. Calling system_one with one choice question returns a typed answer whose probability entries exactly match the question's options, sum to 1.0, and whose choice is the argmax option
  #   3. Answering a score question returns a value between the legend's extremes with one probability per legend entry, and answering a noul question returns a single [0,1] float
  #   4. answer_to_json renders each answer with a `type` field of `choice`, `score`, or `noul` plus the typed fields (choice + probabilities map; score + legend + probabilities; noul + act_probability)
  #
  # ========================================
  Background: User Story
    As a Rust developer
    I want to load a laya checkpoint and answer typed questions in my own Rust program via RLAgent
    So that I get typed decisions (choice/score/noul) with calibrated probabilities, no Python runtime or CLI process needed

  Scenario: A choice answer is typed and normalized
    Given a laya checkpoint is available and I build one choice question
    When I call `system_one` with it
    Then the probability entries exactly match the question's options, sum to 1.0
    And the returned choice is the argmax option

  Scenario: A score answer spans the legend and a noul answer is a [0,1] float
    Given a laya checkpoint is available and I build one score and one noul question
    When I call `system_one` with both
    Then the score lies between the legend's extremes with one probability per legend entry
    And the noul answer is a single float in [0,1]

  Scenario: Options that exceed head_max_len are rejected
    Given a laya checkpoint is available and I build a question whose options exceed head_max_len
    When I call `system_one` with it
    Then it fails with an error naming the question id and head_max_len

  Scenario: answer_to_json renders the shared typed shape
    Given a laya checkpoint is available and I have answered one choice, one score, and one noul question
    When I render each returned answer with `answer_to_json`
    Then each JSON object has a `type` of `choice`, `score`, or `noul` with its typed fields
