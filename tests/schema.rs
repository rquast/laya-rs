/**
 * Feature: spec/features/typed-question-schema.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * All scenarios are weight-free: `render_options` is pure string formatting,
 * and `build_sequence` is exercised with an in-memory WordLevel tokenizer —
 * a Whitespace pre-tokenizer plus a vocab where every word used below has a
 * known id (special tokens: [unk]=0, [PAD]=1, [CLS]=2, [SEP]=3, [MASK]=4).
 */

use std::collections::HashMap;

use rlcd::schema::{build_sequence, render_options, QType, Question, SpecialTokens};
use tokenizers::models::wordlevel::WordLevel;
use tokenizers::pre_tokenizers::whitespace::Whitespace;
use tokenizers::Tokenizer;

/// In-memory WordLevel tokenizer with a Whitespace pre-tokenizer: each
/// whitespace-separated word is one token, with known ids (words added from 5).
fn test_tokenizer(words: &[&str]) -> Tokenizer {
    let mut vocab: HashMap<String, u32> = HashMap::new();
    vocab.insert("<unk>".to_string(), 0);
    vocab.insert("[PAD]".to_string(), 1);
    vocab.insert("[CLS]".to_string(), 2);
    vocab.insert("[SEP]".to_string(), 3);
    vocab.insert("[MASK]".to_string(), 4);
    for (i, w) in words.iter().enumerate() {
        vocab.insert(w.to_string(), 5 + i as u32);
    }
    let model = WordLevel::builder().vocab(vocab).build().expect("word-level model");
    let mut tok = Tokenizer::new(model);
    tok.with_pre_tokenizer(Some(Whitespace::default()));
    tok
}

fn special() -> SpecialTokens {
    SpecialTokens {
        cls: "[CLS]".to_string(),
        sep: "[SEP]".to_string(),
        mask: "[MASK]".to_string(),
        pad: "[PAD]".to_string(),
    }
}

const CLS: u32 = 2;
const SEP: u32 = 3;
const MASK: u32 = 4;

/// Scenario: Choice question with two options yields two markers
#[test]
fn choice_question_with_two_options_yields_two_markers() {
    // @step Given a tokenizer is available and a choice question has options `billing` and `technical` with a string state
    let tok = test_tokenizer(&[
        "choice", "question:", "Which", "team", "should", "handle", "this?",
        "We", "were", "billed", "twice.",
        "billing:", "invoices,", "payments,", "refunds", "technical",
    ]);
    let q = Question {
        qtype: QType::Choice,
        instructions: "Which team should handle this?".to_string(),
        choice_criteria: vec![
            ("billing".to_string(), Some("invoices, payments, refunds".into())),
            ("technical".to_string(), None),
        ],
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };
    let state = serde_json::json!("We were billed twice.");

    // @step When the schema builder runs build_sequence over the question and state
    let built = build_sequence(&tok, &special(), &state, &q, 256, 128);

    // @step Then the built ids start with the CLS token, end with the SEP token, and exactly two marker positions are returned — one per option — each pointing at a MASK token
    assert_eq!(built.ids.first().copied(), Some(CLS), "ids must start with [CLS]");
    assert_eq!(built.ids.last().copied(), Some(SEP), "ids must end with [SEP]");
    assert_eq!(built.markers.len(), 2, "one marker per option");
    for &m in &built.markers {
        assert_eq!(built.ids[m], MASK, "each marker must point at a [MASK] token");
    }
    // the two options are distinct rendered strings, so their marker positions differ
    assert_ne!(built.markers[0], built.markers[1]);
}

/// Scenario: Noul question without criteria uses the default options
#[test]
fn noul_question_without_criteria_uses_the_default_options() {
    // @step Given a noul question has no explicit true or false criteria
    let q = Question {
        qtype: QType::Noul,
        instructions: "Does the user threaten to cancel?".to_string(),
        choice_criteria: vec![],
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };

    // @step When the schema builder renders the question's options
    let opts = render_options(&q);

    // @step Then the rendered options are exactly `false: no, the statement does not hold` and `true: yes, the statement holds`
    assert_eq!(
        opts,
        vec![
            "false: no, the statement does not hold".to_string(),
            "true: yes, the statement holds".to_string(),
        ]
    );
}

/// Scenario: Options exceeding the head budget are capped and pruned
#[test]
fn options_exceeding_the_head_budget_are_capped_and_pruned() {
    // Two 30-word options: each renders as 1 MASK + 30 text tokens, so the two
    // options (62 total) leave head_max_len=40 with opt_budget < 16.
    let w: Vec<String> = (0..60).map(|i| format!("w{i}")).collect();
    let wref: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
    let mut words: Vec<&str> = vec![
        "choice", "question:", "t?", "We", "were", "billed", "twice.",
    ];
    words.extend(wref.iter().copied());
    let tok = test_tokenizer(&words);

    let q = Question {
        qtype: QType::Choice,
        instructions: "t?".to_string(),
        choice_criteria: vec![
            (w[0..30].join(" "), None),
            (w[30..60].join(" "), None),
        ],
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };
    let state = serde_json::json!("We were billed twice.");
    let head_max_len = 40usize;
    let n = 2usize;
    let per = ((head_max_len - 16) / n).max(4);

    // @step Given a choice question's options would leave fewer than 16 head tokens after rendering
    // uncapped option sum = 2 * (1 MASK + 30 words) = 62 > head_max_len = 40, so opt_budget < 16
    let uncapped_option_sum = (1 + 30) * n;
    assert!(uncapped_option_sum > head_max_len, "fixture must trip the head-budget fallback");

    // @step When the schema builder runs build_sequence with the checkpoint's head_max_len and max_len
    let capped = build_sequence(&tok, &special(), &state, &q, 256, head_max_len);
    // both options render longer than the cap, so both hit it
    let span = capped.markers[1] - capped.markers[0];
    assert_eq!(span, per, "each option's full vector is capped to {per}");
    // pruned variant: a tiny max_len drops the second marker
    let pruned = build_sequence(&tok, &special(), &state, &q, 15, head_max_len);

    // @step Then every option is capped at (head_max_len-16)/n tokens (floor 4) and, if the final max_len truncation removes option tokens, the returned marker count is lower than the option count
    assert_eq!(pruned.ids.len(), 15, "ids must be truncated to max_len");
    assert!(
        pruned.markers.len() < n,
        "marker count {} must be lower than option count {n}",
        pruned.markers.len()
    );
}
