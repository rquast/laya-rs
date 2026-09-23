@done
@decision-inference
@decision-engine
@INFER-004
Feature: Build a token sequence with option markers from a typed question
  """
  Port of rl_common.render_options / build_sequence (src/schema.rs). Pure Rust over a HuggingFace `tokenizers` Tokenizer — no model weights required, so it's fully unit-testable with any checkpoint's tokenizer.json. Layout: `[CLS] <type> question: <instructions> [SEP] [MASK] opt0 [MASK] opt1 ... [SEP] <state> [SEP]`. Mask tokens are blanked in instructions/options; string states pass through, object states serialize to JSON. Budgets: per-option 48-token cap, head min-16 fallback (per-option min 4), head min 8, max_len truncation, marker pruning.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. render_options renders choice options as `key` or `key: description` (skipping empty descriptions), score options as `level <i>: <criterion>` in order, and noul as exactly two options `false: <noul_false>` and `true: <noul_true>` with defaults "no, the statement does not hold" / "yes, the statement holds" when absent
  #   2. build_sequence lays out the tokens as `[CLS] <type> question: <instructions> [SEP] [MASK] opt0 [MASK] opt1 ... [SEP] <state> [SEP]` with one marker index per option; the mask token is blanked inside instructions and options, and a string state is used verbatim while an object state is serialized to JSON
  #   3. Token budgets: each option renders as one MASK token plus up to 48 option-text tokens; if the rendered options leave fewer than 16 head tokens, every option's full token vector (mask + text) is capped evenly at (head_max_len-16)/n tokens, with a floor of 4 per option; the head is then truncated to at most max(remaining head budget, 8) tokens; the state is truncated to whatever fits under max_len; and after the final max_len truncation any marker at or beyond max_len is dropped, so the returned marker count can be lower than the option count
  #
  # EXAMPLES:
  #   1. Building a sequence for a choice question with two options and a string state yields ids starting with the CLS token, ending with the SEP token, and exactly two marker positions — one per option — each pointing at a `[MASK]` token
  #   2. Rendering a noul question with no explicit true/false criteria yields exactly two options: `false: no, the statement does not hold` and `true: yes, the statement holds`
  #   3. Building a sequence whose options would leave fewer than 16 head tokens caps every option at (head_max_len-16)/n tokens (floor 4); after the final max_len truncation any marker that no longer fits is dropped, so the returned marker count can be less than the option count
  #
  # ========================================
  Background: User Story
    As a library consumer (RLAgent, Trainer, wasm bindings)
    I want to turn a typed question and a state into the exact token sequence the model consumes
    So that the choice/score/noul contract between the Rust side and the checkpoint stays byte-compatible with the original

  Scenario: Choice question with two options yields two markers
    Given a tokenizer is available and a choice question has options `billing` and `technical` with a string state
    When the schema builder runs build_sequence over the question and state
    Then the built ids start with the CLS token, end with the SEP token, and exactly two marker positions are returned — one per option — each pointing at a MASK token

  Scenario: Noul question without criteria uses the default options
    Given a noul question has no explicit true or false criteria
    When the schema builder renders the question's options
    Then the rendered options are exactly `false: no, the statement does not hold` and `true: yes, the statement holds`

  Scenario: Options exceeding the head budget are capped and pruned
    Given a choice question's options would leave fewer than 16 head tokens after rendering
    When the schema builder runs build_sequence with the checkpoint's head_max_len and max_len
    Then every option is capped at (head_max_len-16)/n tokens (floor 4) and, if the final max_len truncation removes option tokens, the returned marker count is lower than the option count
