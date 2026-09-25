/**
 * Feature: spec/features/checkpoint-resolution.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * `rlcd::model_path::resolve` and `rlcd::download` are native-only (the
 * `wasm32` target has neither). Cache-directory scenarios mutate
 * XDG_CACHE_HOME / RLCD_OFFLINE, which are process-global: they hold
 * ENV_LOCK so the parallel test threads never see another test's env.
 */

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(key: &str, val: Option<&str>) {
    match val {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
}

/// The five files a complete checkpoint holds.
const REQUIRED: &[&str] = &[
    "rl_agent_config.json",
    "encoder/config.json",
    "tokenizer/tokenizer.json",
    "tokenizer/tokenizer_config.json",
    "model.safetensors",
];

fn make_checkpoint(dir: &Path) {
    for file in REQUIRED {
        let p = dir.join(file);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, b"").unwrap();
    }
}

fn tmp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rlcd-check-{name}-{}", std::process::id()))
}

/// Scenario: An explicit checkpoint path is used as-is
#[test]
fn an_explicit_checkpoint_path_is_used_as_is() {
    let _guard = env_lock();

    // @step Given an explicit checkpoint directory path
    let explicit = tmp_root("explicit");
    std::fs::create_dir_all(&explicit).unwrap();

    // @step When the resolver runs with that explicit path
    let resolved = rlcd::model_path::resolve(
        "typed-decisions",
        Some(explicit.clone()),
        Some("typed-decisions"),
        None,
    )
    .expect("explicit path must resolve");

    // @step Then it returns that exact path without consulting the variant, family root, or cache
    assert_eq!(resolved, explicit);
    let _ = std::fs::remove_dir_all(&explicit);
}

/// Scenario: A family-root subfolder is preferred over the cache
#[test]
fn a_family_root_subfolder_is_preferred_over_the_cache() {
    let _guard = env_lock();

    // @step Given a family root containing the typed-decisions subfolder with a model.safetensors
    let root = tmp_root("family");
    let sub = root.join("typed-decisions");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("model.safetensors"), b"").unwrap();

    // @step When the resolver resolves the typed-decisions variant without an explicit path
    let resolved = rlcd::model_path::resolve("typed-decisions", None, Some("typed-decisions"), Some(&root))
        .expect("family root must resolve");

    // @step Then it returns the family-root subfolder
    assert_eq!(resolved, sub);
    let _ = std::fs::remove_dir_all(&root);
}

/// Scenario: A complete cache is used without a download
#[test]
fn a_complete_cache_is_used_without_a_download() {
    let _guard = env_lock();

    // @step Given a cache directory holding all five checkpoint files for the variant
    let cache = tmp_root("cache-complete");
    set_env("XDG_CACHE_HOME", Some(cache.to_str().unwrap()));
    set_env("RLCD_OFFLINE", None);
    let variant_dir = cache.join("rlcd-rs").join("laya-typed-decisions");
    make_checkpoint(&variant_dir);

    // @step When the resolver resolves without an explicit path or family root
    let resolved =
        rlcd::download::download_variant(rlcd::model_path::find("typed-decisions").unwrap())
            .expect("complete cache must resolve without a download");

    // @step Then it returns the cache directory and no file is downloaded
    assert_eq!(resolved, variant_dir);
    set_env("XDG_CACHE_HOME", None);
    let _ = std::fs::remove_dir_all(&cache);
}

/// Scenario: A missing cache file offline is a clear error
#[test]
fn a_missing_cache_file_offline_is_a_clear_error() {
    let _guard = env_lock();

    // @step Given an empty cache with RLCD_OFFLINE set
    let cache = tmp_root("cache-offline");
    std::fs::create_dir_all(&cache).unwrap();
    set_env("XDG_CACHE_HOME", Some(cache.to_str().unwrap()));
    set_env("RLCD_OFFLINE", Some("1"));

    // @step When the resolver resolves the variant
    let err = rlcd::download::download_variant(rlcd::model_path::find("multilingual").unwrap())
        .expect_err("offline with an empty cache must fail");

    // @step Then it fails with an error that names RLCD_OFFLINE and --model
    let msg = format!("{err:?}");
    assert!(msg.contains("RLCD_OFFLINE"), "error should name RLCD_OFFLINE: {msg}");
    assert!(msg.contains("--model"), "error should name --model: {msg}");
    set_env("XDG_CACHE_HOME", None);
    set_env("RLCD_OFFLINE", None);
    let _ = std::fs::remove_dir_all(&cache);
}

/// Scenario: An unknown variant key is an error
#[test]
fn an_unknown_variant_key_is_an_error() {
    // @step Given the variant key "does-not-exist"
    let key = "does-not-exist";

    // @step When the resolver resolves it
    let err = rlcd::model_path::resolve("typed-decisions", None, Some(key), None)
        .expect_err("unknown variant must fail");

    // @step Then it fails with an unknown-variant error
    let msg = format!("{err:?}");
    assert!(msg.contains("unknown model variant"), "error should name the variant: {msg}");
}
