@done
@training-calibration
@decision-engine
@TRAIN-001
Feature: Train (fine-tune) via CLI over a JSONL dataset
  """
  Records deserialize into batching::Record (untagged: Episode{kind, ep, qs, src} | Plain{state, qs, src}); blank lines filtered; parse errors propagate (anyhow). train_jsonl shuffles with seeded rand, chunks 32 records/micro-step, option-order shuffling enabled; per-epoch stdout line `epoch {n}: loss={:.4} reward={:.4} ({steps} steps)`. Deterministic given seed.
  CLI `Train` arm in src/main.rs: builds RlcdConfig {lr, group_size, noise_sigma: sigma, ..Default}, Trainer::load(model_dir), trainer.train_jsonl(dataset, epochs), trainer.save(save_to.unwrap_or(model_dir/model.trained.safetensors)). RlcdConfig defaults: lr 1e-5, group 8, sigma 1.0, w_sph 0.5, w_rps 1.0, log_floor -9.21, td_lambda 1.0, max_tokens 16384, max_seqs 256, seed 0.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. Hyperparameters have defaults: epochs 1, lr 1e-5, group-size 8, sigma 1.0; all are overridable via flags
  #   2. The dataset file is JSONL: one training record per line; blank lines are skipped; each record is either a plain state+typed-questions record or a multi-turn episode (kind: "episode")
  #   3. Each epoch logs one line of mean loss/reward; trained weights are saved to `<model_dir>/model.trained.safetensors` unless --save-to overrides the path
  #
  # EXAMPLES:
  #   1. Running `laya train /path/to/checkpoint train.jsonl` (1 epoch) prints an epoch summary line and writes /path/to/checkpoint/model.trained.safetensors
  #   2. Passing --epochs 3 trains for three shuffled passes over the dataset and logs one epoch line per pass
  #   3. A JSONL line that is not a valid training record (bad JSON) aborts the run with a parse error instead of silently skipping it
  #
  # ========================================
  Background: User Story
    As a ML practitioner
    I want to fine-tune the decision model via `laya train` over a JSONL dataset
    So that the shipped checkpoint is tuned to my domain's decision labels

  Scenario: One epoch trains and saves default-path weights
    Given a checkpoint directory and a one-line JSONL dataset
    When I run `laya train` with default epochs
    Then one epoch summary line is printed
    And the trained weights are saved to `<model_dir>/model.trained.safetensors`

  Scenario: Multiple epochs log one line per pass
    Given a JSONL dataset with two or more records
    When I run `laya train` with `--epochs 3`
    Then three epoch summary lines are printed, one per shuffled pass

  Scenario: A malformed JSONL line aborts with a parse error
    Given a JSONL dataset containing one line that is not a valid record
    When I run `laya train` over it
    Then the run aborts with a parse error instead of silently skipping the line
