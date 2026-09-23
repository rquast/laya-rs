/**
 * Feature: spec/features/language-checkpoint-routing.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * All scenarios are weight-free and deterministic: `laya::route` returns a
 * `Checkpoint` enum with no I/O. (Native-only API: `route` is gated to
 * `cfg(not(target_arch = "wasm32"))`, and `cargo test` runs native.)
 */

use laya::{route, Checkpoint};

/// Scenario: Ordinary English text routes to the English checkpoint
#[test]
fn ordinary_english_text_routes_to_the_english_checkpoint() {
    // @step Given the state text is the English sentence "A customer says a password reset succeeded, but every login attempt still returns 'account locked'."
    let text = "A customer says a password reset succeeded, but every login attempt still returns 'account locked'.";

    // @step When the router runs `route()` over the state text
    let checkpoint = route(text);

    // @step Then the router returns the English checkpoint
    assert_eq!(checkpoint, Checkpoint::English);
}

/// Scenario: Devanagari text routes to the multilingual checkpoint
#[test]
fn devanagari_text_routes_to_the_multilingual_checkpoint() {
    // @step Given the state text is the Devanagari sentence "मुझसे दो बार शुल्क लिया गया"
    let text = "मुझसे दो बार शुल्क लिया गया";

    // @step When the router runs `route()` over the state text
    let checkpoint = route(text);

    // @step Then the router returns the multilingual checkpoint
    assert_eq!(checkpoint, Checkpoint::Multilingual);
}

/// Scenario: French text routes to the multilingual checkpoint
#[test]
fn french_text_routes_to_the_multilingual_checkpoint() {
    // @step Given the state text is the French sentence "Le client signale que la facture de mars a été facturée deux fois."
    let text = "Le client signale que la facture de mars a été facturée deux fois.";

    // @step When the router runs `route()` over the state text
    let checkpoint = route(text);

    // @step Then the router returns the multilingual checkpoint
    assert_eq!(checkpoint, Checkpoint::Multilingual);
}
