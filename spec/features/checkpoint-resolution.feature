@done
@checkpoint-management
@decision-engine
@CHECK-001
Feature: Resolve a checkpoint directory by variant
  """
  src/model_path.rs (native-only): resolve(default_variant_key, explicit, requested_variant, family_root). VARIANTS: typed-decisions (hf convaiinnovations/laya-typed-decisions), multilingual (hf convaiinnovations/laya-multilingual). Order: explicit as-is -> family root subfolder (candidates: hf repo basename then subfolder; must contain model.safetensors) -> download::download_variant (cache $XDG_CACHE_HOME/rlcd-rs/<repo-basename>, 5 REQUIRED_FILES, skip existing, RLCD_OFFLINE -> bail with 'unset it or pass --model').
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. An explicit --model/LAYA_MODEL path wins outright and is used as-is, with no variant lookup, routing, or download
  #   2. Without an explicit path: a family root (--models-root/LAYA_MODELS_ROOT), if given, wins when it contains a recognized subfolder (standalone repo basename first, then hub subfolder name) that holds model.safetensors
  #   3. Failing both, the variant's cached standalone download ($XDG_CACHE_HOME/rlcd-rs/<repo-name>) is returned, downloading missing files from Hugging Face first; RLCD_OFFLINE makes a missing file an error instead of a network fetch
  #
  # EXAMPLES:
  #   1. With a family root that contains the variant's subfolder, `rlcd ask` loads the subfolder checkpoint directly — no cache lookup, no download
  #   2. With a cache directory where the variant's checkpoint files already exist, the run uses them directly and downloads nothing
  #   3. When a checkpoint file is missing from the cache and RLCD_OFFLINE is set, resolution fails with a clear error telling the user to unset it or pass --model
  #
  # ========================================
  Background: User Story
    As a CLI caller (ask/answer paths)
    I want to resolve a laya checkpoint directory for a variant without downloading by hand
    So that the CLI always loads a valid, complete checkpoint for the language it routed to

  Scenario: An explicit checkpoint path is used as-is
    Given an explicit checkpoint directory path
    When the resolver runs with that explicit path
    Then it returns that exact path without consulting the variant, family root, or cache

  Scenario: A family-root subfolder is preferred over the cache
    Given a family root containing the typed-decisions subfolder with a model.safetensors
    When the resolver resolves the typed-decisions variant without an explicit path
    Then it returns the family-root subfolder

  Scenario: A complete cache is used without a download
    Given a cache directory holding all five checkpoint files for the variant
    When the resolver resolves without an explicit path or family root
    Then it returns the cache directory and no file is downloaded

  Scenario: A missing cache file offline is a clear error
    Given an empty cache with RLCD_OFFLINE set
    When the resolver resolves the variant
    Then it fails with an error that names RLCD_OFFLINE and --model

  Scenario: An unknown variant key is an error
    Given the variant key "does-not-exist"
    When the resolver resolves it
    Then it fails with an unknown-variant error
