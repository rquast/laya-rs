/**
 * Feature: spec/features/agent-inference-api.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * All scenarios need a real checkpoint: `RLAgent::load` runs before any of the
 * asserted behavior, and the model weights are not bundled in the repository.
 * They gate on LAYA_TEST_MODEL (checkpoint directory) and skip without it.
 */

use laya::agent::answer_to_json;
use laya::RLAgent;
use laya::schema::{QType, Question};
use serde_json::json;

fn model_dir() -> Option<String> {
    std::env::var("LAYA_TEST_MODEL").ok()
}

fn load_agent() -> Option<RLAgent> {
    let model = model_dir()?;
    RLAgent::load(&model).ok()
}

/// Scenario: A choice answer is typed and normalized
#[test]
fn a_choice_answer_is_typed_and_normalized() {
    // @step Given a laya checkpoint is available and I build one choice question
    let agent = match load_agent() {
        Some(a) => a,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
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
    let state = json!("We were billed twice for March. Please refund the duplicate.");

    // @step When I call `system_one` with it
    let answers = agent
        .system_one(&state, &[("q".to_string(), q)])
        .expect("system_one must succeed");

    // @step Then the probability entries exactly match the question's options, sum to 1.0
    let laya::Answer::Choice { probabilities, .. } = &answers[0].1 else {
        panic!("expected a Choice answer");
    };
    let keys: Vec<&str> = probabilities.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["billing", "technical"], "probabilities must name the options in order: {keys:?}");
    let sum: f32 = probabilities.iter().map(|(_, p)| *p).sum();
    assert!((sum - 1.0).abs() < 1e-3, "probabilities must sum to 1.0, got {sum}");

    // @step And the returned choice is the argmax option
    let laya::Answer::Choice { choice, probabilities, .. } = &answers[0].1 else {
        panic!("expected a Choice answer");
    };
    let argmax = probabilities
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(k, _)| k.clone())
        .unwrap();
    assert_eq!(choice, &argmax, "choice must be the argmax option");
}

/// Scenario: A score answer spans the legend and a noul answer is a [0,1] float
#[test]
fn a_score_answer_spans_the_legend_and_a_noul_answer_is_a_0_1_float() {
    // @step Given a laya checkpoint is available and I build one score and one noul question
    let agent = match load_agent() {
        Some(a) => a,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
    let score_q = Question {
        qtype: QType::Score,
        instructions: "How severe is the billing error?".to_string(),
        choice_criteria: vec![],
        score_criteria: vec![
            "minor cosmetic issue".to_string(),
            "wrong amount, single invoice".to_string(),
            "repeated overbilling".to_string(),
        ],
        noul_true: None,
        noul_false: None,
    };
    let noul_q = Question {
        qtype: QType::Noul,
        instructions: "Does the customer threaten to cancel?".to_string(),
        choice_criteria: vec![],
        score_criteria: vec![],
        noul_true: Some("yes, explicit cancellation threat".into()),
        noul_false: Some("no threat present".into()),
    };
    let state = json!("We were billed twice for March. Please refund the duplicate.");

    // @step When I call `system_one` with both
    let answers = agent
        .system_one(&state, &[("s".to_string(), score_q), ("n".to_string(), noul_q)])
        .expect("system_one must succeed");

    // @step Then the score lies between the legend's extremes with one probability per legend entry
    let laya::Answer::Score { score, probabilities, legend, .. } = &answers[0].1 else {
        panic!("expected a Score answer");
    };
    let n = legend.len();
    assert_eq!(probabilities.len(), n, "one probability per legend entry");
    assert!((0.0..=3.0).contains(score), "score {score} must lie within the legend range 0..3");

    // @step And the noul answer is a single float in [0,1]
    let laya::Answer::Noul { noul, .. } = &answers[1].1 else {
        panic!("expected a Noul answer");
    };
    assert!((0.0..=1.0).contains(noul), "noul {noul} must be in [0,1]");
}

/// Scenario: Options that exceed head_max_len are rejected
#[test]
fn options_that_exceed_head_max_len_are_rejected() {
    // @step Given a laya checkpoint is available and I build a question whose options exceed head_max_len
    let agent = match load_agent() {
        Some(a) => a,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
    // 300 options of 48 words each (~49 tokens per option): even a generous
    // head_max_len/max_len checkpoint cannot hold all of them, so the marker
    // count after truncation can never equal the option count.
    let many: Vec<(String, Option<String>)> = (0..300)
        .map(|i| {
            let words = (0..48).map(|w| format!("word{i}_{w}")).collect::<Vec<_>>().join(" ");
            (format!("option{i}"), Some(words))
        })
        .collect();
    let q = Question {
        qtype: QType::Choice,
        instructions: "Pick one.".to_string(),
        choice_criteria: many,
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };
    let state = json!("state");

    // @step When I call `system_one` with it
    let result = agent.system_one(&state, &[("big".to_string(), q)]);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("an option count that cannot fit must be rejected"),
    };

    // @step Then it fails with an error naming the question id and head_max_len
    let msg = format!("{err:?}");
    assert!(msg.contains("big"), "error should name the question id: {msg}");
    assert!(msg.contains("head_max_len"), "error should name head_max_len: {msg}");
}

/// Scenario: answer_to_json renders the shared typed shape
#[test]
fn answer_to_json_renders_the_shared_typed_shape() {
    // @step Given a laya checkpoint is available and I have answered one choice, one score, and one noul question
    let agent = match load_agent() {
        Some(a) => a,
        None => return, // skipped: no checkpoint available (LAYA_TEST_MODEL unset)
    };
    let choice_q = Question {
        qtype: QType::Choice,
        instructions: "Which team should handle this?".to_string(),
        choice_criteria: vec![("billing".to_string(), None), ("technical".to_string(), None)],
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };
    let score_q = Question {
        qtype: QType::Score,
        instructions: "How severe is this?".to_string(),
        choice_criteria: vec![],
        score_criteria: vec!["low".to_string(), "high".to_string()],
        noul_true: None,
        noul_false: None,
    };
    let noul_q = Question {
        qtype: QType::Noul,
        instructions: "Does it hold?".to_string(),
        choice_criteria: vec![],
        score_criteria: vec![],
        noul_true: None,
        noul_false: None,
    };
    let state = json!("state");
    let answers = agent
        .system_one(&state, &[("c".into(), choice_q), ("s".into(), score_q), ("n".into(), noul_q)])
        .expect("system_one must succeed");

    // @step When I render each returned answer with `answer_to_json`
    let rendered: Vec<serde_json::Value> = answers.into_iter().map(|(_, a)| answer_to_json(a)).collect();

    // @step Then each JSON object has a `type` of `choice`, `score`, or `noul` with its typed fields
    assert_eq!(rendered[0]["type"], "choice");
    assert!(rendered[0]["probabilities"].get("billing").is_some());
    assert!(rendered[0]["probabilities"].get("technical").is_some());
    assert!(rendered[0].get("choice").is_some());
    assert!(rendered[0].get("confidence").is_some());
    assert!(rendered[0].get("act_probability").is_some());

    assert_eq!(rendered[1]["type"], "score");
    assert!(rendered[1].get("score").is_some());
    assert!(rendered[1]["legend"].get("0").is_some());
    assert!(rendered[1]["legend"].get("1").is_some());

    assert_eq!(rendered[2]["type"], "noul");
    assert!(rendered[2].get("noul").is_some());
    assert!(rendered[2].get("act_probability").is_some());
}
