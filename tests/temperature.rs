/**
 * Feature: spec/features/temperature-calibration.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * `fit_temperature` / `fit_temperatures` are pure, weight-free grid searches over
 * already-computed logits — no checkpoint, model, network, or env is needed, so
 * no scenario is gated or skipped.
 */

use laya::schema::QType;
use laya::train::{fit_temperature, fit_temperatures};

/// Scenario: Two-option samples share one bucket temperature
#[test]
fn two_option_samples_share_one_bucket_temperature() {
    // @step Given two choice samples with two options each whose targets are the argmax
    let samples = vec![
        (QType::Choice, vec![4.0f32, -4.0], 0usize),
        (QType::Choice, vec![4.0f32, -4.0], 0usize),
    ];

    // @step When I fit temperatures over both samples
    let temps = fit_temperatures(&samples);

    // @step Then the result contains exactly one bucket, choice:2
    assert_eq!(temps.len(), 1, "expected exactly one bucket: {temps:?}");
    let t = temps.get("choice:2").expect("expected a choice:2 bucket: {temps:?}");

    // @step And its temperature is the coldest candidate on the grid, 0.05
    assert!((t - 0.05).abs() < 1e-5, "expected 0.05, got {t}");
}

/// Scenario: A non-argmax target fits the hottest temperature
#[test]
fn a_non_argmax_target_fits_the_hottest_temperature() {
    // @step Given a two-option choice sample whose target is not the argmax
    let samples = vec![(QType::Choice, vec![4.0f32, -4.0], 1usize)];

    // @step When I fit temperatures over it
    let temps = fit_temperatures(&samples);

    // @step Then the choice:2 temperature is the hottest candidate on the grid, 20.0
    let t = temps.get("choice:2").expect("expected a choice:2 bucket: {temps:?}");
    assert!((t - 20.0).abs() < 1e-5, "expected 20.0, got {t}");
}

/// Scenario: Different option counts fit independent bucket temperatures
#[test]
fn different_option_counts_fit_independent_bucket_temperatures() {
    // @step Given a two-option choice sample whose target is the argmax and a three-option choice sample whose target is not the argmax
    let samples = vec![
        (QType::Choice, vec![4.0f32, -4.0], 0usize),
        (QType::Choice, vec![3.0f32, -1.0, -2.0], 2usize),
    ];

    // @step When I fit temperatures over both samples
    let temps = fit_temperatures(&samples);

    // @step Then choice:2 and choice:3-5 are returned as separate buckets
    assert!(temps.get("choice:2").is_some(), "expected choice:2: {temps:?}");
    assert!(temps.get("choice:3-5").is_some(), "expected choice:3-5: {temps:?}");
    assert_eq!(temps.len(), 2, "expected exactly two buckets: {temps:?}");

    // @step And choice:2 is 0.05 while choice:3-5 is 20.0
    assert!((temps["choice:2"] - 0.05).abs() < 1e-5, "choice:2: {}", temps["choice:2"]);
    assert!((temps["choice:3-5"] - 20.0).abs() < 1e-5, "choice:3-5: {}", temps["choice:3-5"]);
}

/// Scenario: A single-sample fit matches its bucket fit
#[test]
fn a_single_sample_fit_matches_its_bucket_fit() {
    // @step Given a two-option choice sample whose target is the argmax
    let logits = vec![4.0f32, -4.0];
    let target = 0usize;

    // @step When I fit the single-sample temperature and the bucketed fit for the same sample
    let single = fit_temperature(&logits, target);
    let bucketed = fit_temperatures(&[(QType::Choice, logits.clone(), target)]);

    // @step Then both return 0.05
    assert!((single - 0.05).abs() < 1e-5, "single-sample: {single}");
    assert!(
        (bucketed["choice:2"] - 0.05).abs() < 1e-5,
        "bucketed: {}",
        bucketed["choice:2"]
    );
}
