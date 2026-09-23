@done
@decision-engine
@training-calibration
@TRAIN-002
Feature: RLCD training loop
  """
  Record -> model-input pipeline (src/batching.rs) + RLCD trainer (src/train.rs). encode_record turns a Record (Plain{state, qs} | Episode{ep: {ctx, turns, y}, qs}) into Items: one per question (or per episode prefix, noul over ctx+conversation with target [1-y, y], label round(y)); non-score questions get their option order shuffled (train mode) and soft/one-hot targets + labels remapped into the shuffled order; items whose markers miss the budget are dropped. collate_items pads into [n, l] ids/attention + [n, kmax] marker_pos/marker_mask/target + qtype/label. pack_groups keeps each record's items together under padded-token (max_tokens) and sequence-count (max_seqs) budgets; a record alone over budget is split into ceil chunks. make_token_batches length-buckets record indices under the same budgets, shuffling at chunk and batch level. The REINFORCE step (src/train.rs, best-effort reconstruction — see module docs): detach logits, add N(0, sigma^2) noise per group member over real option slots, reward each row with the strictly-proper scoring rule (log + spherical + RPS for score) against the target (TD(lambda)-bootstrapped for episodes, g_j=(1-lambda)*p_true[next]+lambda*g_next), baseline = group mean of the g rewards (GRPO-style), loss = mean over rows of sum_c((x-c_mu)^2/(2 sigma^2)) * advantage, plus a weighted-Cross-entropy act-head auxiliary loss (target escalate=1 when the clean argmax choice is wrong; wrong answers weighted by act_costs["escalate"] floor 0.1, unlabeled items weight 0); AdamW step per sub-batch. Pure pipeline functions are unit-testable with an in-memory WordLevel tokenizer; Trainer::load/train_step/train_jsonl need a real checkpoint (gate on LAYA_TEST_MODEL, skip otherwise). RlcdConfig defaults: lr 1e-5, group 8, sigma 1.0, w_sph 0.5, w_rps 1.0, log_floor -9.21, td_lambda 1.0, act_loss_weight 1.0, max_tokens 16384, max_seqs 256, seed 0.
  """

  Background: User Story
    As a ML practitioner
    I want to run the RLCD training loop — record encoding, batching, and the REINFORCE-with-Gaussian-exploration update
    So that the checkpoint improves on my domain records before it is deployed

  Scenario: Collate pads mixed-length items into batch tensors
    Given two items have ids of different lengths and different option counts
    When the pipeline collates the two items into a batch
    Then ids are padded to a common row length with the pad id and attention 0 beyond each item's real length, marker slots are padded to the max option count with mask 0, and each row carries its qtype index and label

  Scenario: Option shuffling remaps targets and labels during training
    Given a plain record has a two-option choice question with label y=1 and a seeded RNG is available in train mode
    When the pipeline encodes the record with option shuffling enabled
    Then the item's option text appears in a shuffled order while the soft target and the label both still point at the same semantic option in that new order

  Scenario: Record groups stay together under the padded-token budget
    Given three record groups each fit alone under the padded-token and sequence budgets
    When the pipeline packs the groups into sub-batches
    Then all three record groups land in a single sub-batch and every record's items stay together (empty groups are dropped)
    And shrinking the token budget so one record group alone exceeds it splits only that record's items into ceil-sized chunks while the other records' items each stay intact in their own sub-batch

  Scenario: Token batches respect the padded-token budget
    Given eight records have lengths [300, 300, 200, 200, 100, 100, 100, 100] with one sequence each
    When the pipeline builds length-bucketed token batches under a 1000-token, 8-sequence budget
    Then every batch stays within the padded-token budget, every record appears in exactly one batch, and the batch order is shuffled

  Scenario: Episode records yield one item per sampled prefix
    Given an episode record has a 3-turn conversation, outcome y=0.8, and the checkpoint allows 6 prefixes
    When the pipeline encodes the episode record
    Then exactly 3 noul items are returned, one per prefix length 1..3, each with target [0.2, 0.8], label 1, and ep_len 3
    And restricting the episode to 2 sampled prefixes yields exactly 2 items at the evenly spaced prefix lengths 1 and 3

  Scenario: Episode targets are bootstrapped with TD lambda
    Given a batch has three episode prefix items of one record (raw outcome target [0.2, 0.8]) plus one non-episode item, and the policy's P(true) per item is [0.2, 0.5, 0.9]
    When the pipeline computes TD(lambda) targets for the batch with lambda 0.5
    Then the episode items' targets become [0.675, 0.85, 0.8], walking backward from the final prefix and blending the policy's next-step P(true) with the previous target, while the non-episode item's target is left untouched
