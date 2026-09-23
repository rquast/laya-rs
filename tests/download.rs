/**
 * Feature: spec/features/checkpoint-download.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * `laya::download` is native-only (`#[cfg(not(target_arch = "wasm32"))]`).
 * Cache-directory scenarios mutate XDG_CACHE_HOME / LAYA_OFFLINE, which are
 * process-global: they hold ENV_LOCK so parallel test threads never see
 * another test's env. The fresh-cache scenario downloads the real checkpoint
 * (several hundred MB) from Hugging Face, so it only runs when
 * LAYA_TEST_DOWNLOAD is set.
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

fn tmp_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("laya-dl-{name}-{}", std::process::id()))
}

/// The five files a complete checkpoint holds (mirrors `REQUIRED_FILES`).
const REQUIRED: &[&str] = &[
    "rl_agent_config.json",
    "encoder/config.json",
    "tokenizer/tokenizer.json",
    "tokenizer/tokenizer_config.json",
    "model.safetensors",
];

fn make_checkpoint(dir: &Path, marker: &str) {
    for file in REQUIRED {
        let p = dir.join(file);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, marker.as_bytes()).unwrap();
    }
}

fn variant(key: &str) -> &'static laya::model_path::VariantDef {
    laya::model_path::find(key).unwrap()
}

/// Scenario: A fresh cache downloads all five files
#[test]
fn a_fresh_cache_downloads_all_five_files() {
    // @step Given a cache directory without the typed-decisions checkpoint
    if std::env::var("LAYA_TEST_DOWNLOAD").is_err() {
        return; // skipped: real download (hundreds of MB) requires opt-in
    }
    let _guard = env_lock();
    let cache = tmp_root("fresh");
    let _ = std::fs::remove_dir_all(&cache);
    set_env("XDG_CACHE_HOME", Some(cache.to_str().unwrap()));
    set_env("LAYA_OFFLINE", None);

    // @step When I request the typed-decisions variant download over the network (opt-in via LAYA_TEST_DOWNLOAD; the real checkpoint is several hundred MB)
    let resolved = laya::download::download_variant(&variant("typed-decisions"))
        .expect("download must succeed when LAYA_TEST_DOWNLOAD is set");

    // @step Then all five checkpoint files are present under the cache's laya-typed-decisions directory
    let dir = cache.join("laya-rs").join("laya-typed-decisions");
    for file in REQUIRED {
        assert!(dir.join(file).is_file(), "missing {file} under {}", dir.display());
    }

    // @step And the returned directory is that cache path
    assert_eq!(resolved, dir);
    set_env("XDG_CACHE_HOME", None);
    let _ = std::fs::remove_dir_all(&cache);
}

/// Scenario: A fully cached download touches nothing
#[test]
fn a_fully_cached_download_touches_nothing() {
    let _guard = env_lock();

    // @step Given a cache directory holding all five checkpoint files
    let cache = tmp_root("full");
    let _ = std::fs::remove_dir_all(&cache);
    set_env("XDG_CACHE_HOME", Some(cache.to_str().unwrap()));
    set_env("LAYA_OFFLINE", Some("1")); // any missing file would now be a hard error
    let variant_dir = cache.join("laya-rs").join("laya-multilingual");
    make_checkpoint(&variant_dir, "cached-bytes");
    let before: std::collections::HashMap<PathBuf, Vec<u8>> = REQUIRED
        .iter()
        .map(|f| {
            let p = variant_dir.join(f);
            (p.clone(), std::fs::read(&p).unwrap())
        })
        .collect();

    // @step When I request the variant download with LAYA_OFFLINE set
    let resolved = laya::download::download_variant(&variant("multilingual"))
        .expect("fully cached + offline must succeed without network");

    // @step Then it succeeds without downloading anything
    assert_eq!(resolved, variant_dir);

    // @step And the cached files are byte-identical to what was there before
    for (p, content) in &before {
        let now = std::fs::read(p).unwrap();
        assert_eq!(now, *content, "file changed: {}", p.display());
    }
    set_env("XDG_CACHE_HOME", None);
    set_env("LAYA_OFFLINE", None);
    let _ = std::fs::remove_dir_all(&cache);
}

/// Scenario: Offline with a partial cache is an actionable error
#[test]
fn offline_with_a_partial_cache_is_an_actionable_error() {
    let _guard = env_lock();

    // @step Given a cache holding only some of the checkpoint files and LAYA_OFFLINE set
    let cache = tmp_root("partial");
    let _ = std::fs::remove_dir_all(&cache);
    set_env("XDG_CACHE_HOME", Some(cache.to_str().unwrap()));
    set_env("LAYA_OFFLINE", Some("1"));
    let variant_dir = cache.join("laya-rs").join("laya-multilingual");
    std::fs::create_dir_all(&variant_dir).unwrap();
    std::fs::write(variant_dir.join("rl_agent_config.json"), b"{}").unwrap();

    // @step When I request the variant download
    let err = laya::download::download_variant(&variant("multilingual"))
        .expect_err("partial cache + offline must fail");

    // @step Then it fails with an error naming the first missing file, LAYA_OFFLINE, and --model
    let msg = format!("{err:?}");
    assert!(msg.contains("encoder/config.json"), "first missing file: {msg}");
    assert!(msg.contains("LAYA_OFFLINE"), "must name LAYA_OFFLINE: {msg}");
    assert!(msg.contains("--model"), "must name --model: {msg}");
    // No `.part` sibling may exist: nothing partial was fetched before the error.
    assert!(!variant_dir.join("encoder/config.json.part").exists(), "no .part may survive");
    set_env("XDG_CACHE_HOME", None);
    set_env("LAYA_OFFLINE", None);
    let _ = std::fs::remove_dir_all(&cache);
}
