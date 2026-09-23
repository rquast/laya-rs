@done
@checkpoint-management
@decision-engine
@CHECK-002
Feature: Download a checkpoint into the local cache
  """
  src/download.rs (native-only; ureq 2 sync HTTP — the browser demo uses the JS fetch API instead): REQUIRED_FILES (18); cache_dir() XDG_CACHE_HOME or ~/.cache + laya-rs (22); variant_cache_dir = cache/<hf_repo basename> (37); download_variant (86) loops REQUIRED_FILES — skip if dest.is_file(), LAYA_OFFLINE+missing -> bail, else fetch_to_file; fetch_to_file (40): GET huggingface.co/<repo>/resolve/main/<file> to a .part sibling, 64KiB reads, Content-Length check (short read -> remove .part + bail 'short read for {url}: got {written} bytes, expected {len}'), atomic rename, stderr progress '<name> ... done (N bytes)'.
  """

  # ========================================
  # EXAMPLE MAPPING CONTEXT
  # ========================================
  #
  # BUSINESS RULES:
  #   1. The five required files (rl_agent_config.json, encoder/config.json, tokenizer/tokenizer.json, tokenizer/tokenizer_config.json, model.safetensors) are downloaded into $XDG_CACHE_HOME/laya-rs/<repo-basename> (default ~/.cache/laya-rs), one per Hugging Face URL
  #   2. Each file is written to a .part sibling first and atomically renamed into place after the Content-Length (when advertised) is matched; a short read deletes the .part and aborts, so a killed or truncated download never leaves a checkpoint that looks complete
  #   3. Files already present in the cache are skipped; the download is incremental — re-running after a successful download touches nothing
  #
  # EXAMPLES:
  #   1. On a fresh cache, requesting the typed-decisions variant fetches all five files from huggingface.co into ~/.cache/laya-rs/laya-typed-decisions and returns that directory
  #   2. With all five files already cached, resolving the variant does not touch the network at all
  #
  # ========================================
  Background: User Story
    As a CLI caller on a fresh machine
    I want to have a variant's checkpoint downloaded into the local cache
    So that `laya ask`/`answer` work with zero manual setup on a fresh machine

  Scenario: A fresh cache downloads all five files
    Given a cache directory without the typed-decisions checkpoint
    When I request the typed-decisions variant download over the network (opt-in via LAYA_TEST_DOWNLOAD; the real checkpoint is several hundred MB)
    Then all five checkpoint files are present under the cache's laya-typed-decisions directory
    And the returned directory is that cache path

  Scenario: A fully cached download touches nothing
    Given a cache directory holding all five checkpoint files
    When I request the variant download with LAYA_OFFLINE set
    Then it succeeds without downloading anything
    And the cached files are byte-identical to what was there before

  Scenario: Offline with a partial cache is an actionable error
    Given a cache holding only some of the checkpoint files and LAYA_OFFLINE set
    When I request the variant download
    Then it fails with an error naming the first missing file, LAYA_OFFLINE, and --model
