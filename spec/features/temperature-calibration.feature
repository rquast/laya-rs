@done
@training-calibration
@decision-engine
@TRAIN-003
Feature: Fit per-(question-type, option-count) calibration temperatures
  """
  src/train.rs: fit_temperature(logits, target) (341) grid 0.05..20.0 step 0.05 (400 candidates), argmin of single-sample NLL (-ln p_target after softmax of logits/t, p clamped at 1e-12), default best_t 1.0; fit_temperatures(samples: &[(QType, Vec<f32>, usize)]) (359) groups by batching::temp_bucket(qtype, logits.len()) (bucket sizes 2 / 3-5 / 6-10 / 11+), per bucket argmin of MEAN NLL over the same grid; returns HashMap<String, f32> (bucket -> temperature). Pure: no I/O, no model. Used at runtime by RLAgent::system_one (temperature_by_options lookup, fallback to per-qtype temperature then 1.0).
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. Each temperature is the 1-D grid minimum over 0.05..20.0 in steps of 0.05 (400 candidates) that minimizes the mean NLL of the samples in its bucket; temperatures are fit independently per bucket
  #   2. Samples group into buckets by temp_bucket(qtype, option-count) — 'choice:2', 'choice:3-5', 'choice:6-10', 'choice:11+' (and the same sizes for score/noul) — so one temperature is returned per bucket that has at least one sample
  #   3. fit_temperatures is a pure, weight-free post-hoc pass over already-computed logits: no checkpoint, model, or network is involved
  #
  # EXAMPLES:
  #   1. Two choice samples with two options each land in one 'choice:2' bucket and return a single shared temperature for that bucket
  #   2. A sample whose target is NOT the argmax (e.g. logits [4.0, -4.0] with the second option as target) fits a temperature well above 1.0, because flattening the logits raises the target's probability
  #   3. Samples with different option counts land in different buckets — two-option choice samples in 'choice:2', three-option choice samples in 'choice:3-5' — each returning its own temperature
  #
  # ========================================
  Background: User Story
    As a ML practitioner
    I want to fit per-(question-type, option-count) calibration temperatures over a labeled eval set via fit_temperatures
    So that the checkpoint's probability outputs are calibrated on my domain without retraining

  Scenario: Two-option samples share one bucket temperature
    Given two choice samples with two options each whose targets are the argmax
    When I fit temperatures over both samples
    Then the result contains exactly one bucket, choice:2
    And its temperature is the coldest candidate on the grid, 0.05

  Scenario: A non-argmax target fits the hottest temperature
    Given a two-option choice sample whose target is not the argmax
    When I fit temperatures over it
    Then the choice:2 temperature is the hottest candidate on the grid, 20.0

  Scenario: Different option counts fit independent bucket temperatures
    Given a two-option choice sample whose target is the argmax and a three-option choice sample whose target is not the argmax
    When I fit temperatures over both samples
    Then choice:2 and choice:3-5 are returned as separate buckets
    And choice:2 is 0.05 while choice:3-5 is 20.0

  Scenario: A single-sample fit matches its bucket fit
    Given a two-option choice sample whose target is the argmax
    When I fit the single-sample temperature and the bucketed fit for the same sample
    Then both return 0.05
