/**
 * Feature: spec/features/rlcd-training-loop.feature
 *
 * This test file validates the acceptance criteria defined in the feature file.
 * Scenarios map directly to Gherkin scenarios.
 *
 * All scenarios are weight-free: they exercise the pure pipeline functions in
 * `src/batching.rs` (`collate_items`, `encode_record`, `pack_groups`,
 * `make_token_batches`, `td_lambda_targets`) with an in-memory WordLevel
 * tokenizer (special tokens: <unk>=0, [PAD]=1, [CLS]=2, [SEP]=3, [MASK]=4;
 * every user word gets a unique id in order of first appearance) — the same
 * harness pattern as tests/schema.rs. `Trainer::load`/`train_step`/
 * `train_jsonl` require a real checkpoint and stay covered by tests/train.rs
 * (CLI path, gated on LAYA_TEST_MODEL).
 */

use laya::batching::{
    collate_items, encode_record, make_token_batches, pack_groups, td_lambda_targets, Item,
    Record,
};
use laya::schema::{QType, SpecialTokens};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::collections::HashMap;
use tokenizers::models::wordlevel::WordLevel;
use tokenizers::pre_tokenizers::whitespace::WhitespaceSplit;
use tokenizers::Tokenizer;

const MASK: u32 = 4;

fn special() -> SpecialTokens {
    SpecialTokens {
        cls: "[CLS]".to_string(),
        sep: "[SEP]".to_string(),
        mask: "[MASK]".to_string(),
        pad: "[PAD]".to_string(),
    }
}

/// In-memory whitespace tokenizer: whitespace-separated words are single tokens.
/// Every word (or whitespace-free chunk, e.g. a compact JSON state string) in
/// `strings` gets a unique id in order of first appearance (after the five
/// fixed special-token ids).
///
/// Uses `WhitespaceSplit` (split on whitespace only) rather than `Whitespace`
/// (which also splits on punctuation, `\w+|[^\w\s]+`): a compact JSON state
/// string contains no whitespace and must remain ONE token so it is lookable
/// in the vocab by its full string.
fn test_tokenizer(strings: &[&str]) -> Tokenizer {
    let mut vocab: HashMap<String, u32> = HashMap::new();
    vocab.insert("<unk>".to_string(), 0);
    vocab.insert("[PAD]".to_string(), 1);
    vocab.insert("[CLS]".to_string(), 2);
    vocab.insert("[SEP]".to_string(), 3);
    vocab.insert("[MASK]".to_string(), 4);
    for s in strings {
        for w in s.split_whitespace() {
            if !vocab.contains_key(w) {
                vocab.insert(w.to_string(), vocab.len() as u32);
            }
        }
    }
    let model = WordLevel::builder().vocab(vocab).build().expect("word-level model");
    let mut tok = Tokenizer::new(model);
    tok.with_pre_tokenizer(Some(WhitespaceSplit {}));
    tok
}

/// Synthetic item with `len` distinct ids and one marker at position 0.
fn item(len: usize, uid: i64, base: u32) -> Item {
    Item {
        ids: (base..base + len as u32).collect(),
        markers: vec![0],
        qtype: QType::Choice,
        target: vec![1.0],
        label: 0,
        episode: false,
        ep_step: 0,
        ep_len: 1,
        rec_uid: uid,
    }
}

/// Field-wise item equality (`Item` doesn't derive `PartialEq`).
fn same_items(a: &[Item], b: &[Item]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| {
            x.ids == y.ids
                && x.markers == y.markers
                && x.qtype == y.qtype
                && x.target == y.target
                && x.label == y.label
                && x.episode == y.episode
                && x.ep_step == y.ep_step
                && x.ep_len == y.ep_len
                && x.rec_uid == y.rec_uid
        })
}

/// Scenario: Collate pads mixed-length items into batch tensors
#[test]
fn collate_pads_mixed_length_items_into_batch_tensors() {
    // @step Given two items have ids of different lengths and different option counts
    let a = Item {
        ids: vec![10, 11, 12, 13, 14], // 5 real tokens, 2 options
        markers: vec![1, 3],
        qtype: QType::Choice,
        target: vec![0.3, 0.7],
        label: 1,
        episode: false,
        ep_step: 0,
        ep_len: 1,
        rec_uid: 1,
    };
    let b = Item {
        ids: vec![20, 21, 22], // 3 real tokens, 3 options
        markers: vec![0, 1, 2],
        qtype: QType::Score,
        target: vec![0.1, 0.6, 0.3],
        label: 0,
        episode: false,
        ep_step: 0,
        ep_len: 1,
        rec_uid: 2,
    };
    let pad_id = 999u32;

    // @step When the pipeline collates the two items into a batch
    let c = collate_items(&[&a, &b], pad_id);

    // @step Then ids are padded to a common row length with the pad id and attention 0 beyond each item's real length, marker slots are padded to the max option count with mask 0, and each row carries its qtype index and label
    assert_eq!(c.n, 2);
    assert_eq!(c.l, 5, "row length must be the max item length");
    assert_eq!(c.kmax, 3, "marker slot count must be the max option count");
    assert_eq!(
        c.ids,
        vec![10, 11, 12, 13, 14, 20, 21, 22, 999, 999],
        "ids row-major, pad id beyond each item's real length"
    );
    assert_eq!(
        c.attention_mask,
        vec![1, 1, 1, 1, 1, 1, 1, 1, 0, 0],
        "attention 1 for real tokens, 0 on padding"
    );
    assert_eq!(c.marker_pos, vec![1, 3, 0, 0, 1, 2]);
    assert_eq!(
        c.marker_mask,
        vec![1.0, 1.0, 0.0, 1.0, 1.0, 1.0],
        "mask 0 beyond each item's real option count"
    );
    assert_eq!(
        c.target,
        vec![0.3, 0.7, 0.0, 0.1, 0.6, 0.3],
        "target row holds the soft target, 0.0 on padded slots"
    );
    assert_eq!(c.qtype, vec![QType::Choice.as_index(), QType::Score.as_index()]);
    assert_eq!(c.label, vec![1, 0]);
}

/// Scenario: Option shuffling remaps targets and labels during training
#[test]
fn option_shuffling_remaps_targets_and_labels_during_training() {
    // @step Given a plain record has a two-option choice question with label y=1 and a seeded RNG is available in train mode
    let tok = test_tokenizer(&[
        "choice question: Which team?",
        "billing",
        "technical",
        "We were billed twice.",
    ]);
    let rec: Record = serde_json::from_str(
        r#"{"state": "We were billed twice.",
            "qs": [{"type": "choice", "instructions": "Which team?",
                    "criteria": ["billing", "technical"], "y": 1}]}"#,
    )
    .expect("parse plain record");
    let billing_id = tok.token_to_id("billing").expect("billing in vocab");
    let technical_id = tok.token_to_id("technical").expect("technical in vocab");

    // Unshuffled reference (train=false, no RNG): option 0 = billing, option 1 = technical.
    let ref_items = encode_record(&tok, &special(), &rec, 256, 128, 6, 42, None::<&mut StdRng>, false);
    assert_eq!(ref_items.len(), 1, "one item per question");
    let it_ref = &ref_items[0];
    assert_eq!(it_ref.ids[it_ref.markers[0] + 1], billing_id, "unshuffled: marker 0 = billing");
    assert_eq!(it_ref.ids[it_ref.markers[1] + 1], technical_id, "unshuffled: marker 1 = technical");

    let mut rng = StdRng::seed_from_u64(7);

    // @step When the pipeline encodes the record with option shuffling enabled
    let items = encode_record(&tok, &special(), &rec, 256, 128, 6, 42, Some(&mut rng), true);
    assert_eq!(items.len(), 1);
    let it = &items[0];
    assert_eq!(it.markers.len(), 2, "both options still present");
    for &m in &it.markers {
        assert_eq!(it.ids[m], MASK, "each marker still points at a [MASK] token");
    }

    // @step Then the item's option text appears in a shuffled order while the soft target and the label both still point at the same semantic option in that new order
    let opt0_word = it.ids[it.markers[0] + 1];
    let opt1_word = it.ids[it.markers[1] + 1];
    let mut pair = [opt0_word, opt1_word];
    pair.sort();
    assert_eq!(
        pair,
        [billing_id.min(technical_id), billing_id.max(technical_id)],
        "the two option texts must be the same set, possibly reordered"
    );
    // y=1 -> `technical`: both the label and the one-hot target must follow the
    // semantic option into whatever slot the shuffle put it in.
    let technical_pos = if opt0_word == technical_id { 0 } else { 1 };
    assert_eq!(
        it.label, technical_pos as i64,
        "label must point at `technical` in the shuffled order"
    );
    assert_eq!(
        it.target,
        vec![
            if technical_pos == 0 { 1.0 } else { 0.0 },
            if technical_pos == 1 { 1.0 } else { 0.0 },
        ],
        "one-hot target must point at `technical` in the shuffled order"
    );
}

/// Scenario: Record groups stay together under the padded-token budget
#[test]
fn record_groups_stay_together_under_the_padded_token_budget() {
    // @step Given three record groups each fit alone under the padded-token and sequence budgets
    let group_a = (0..2).map(|i| item(20, 1, 100 + i)).collect::<Vec<_>>(); // 2 x 20 = 40
    let group_b = (0..1).map(|i| item(5, 2, 200 + i)).collect::<Vec<_>>(); // 1 x 5 = 5
    let group_c = (0..1).map(|i| item(6, 3, 300 + i)).collect::<Vec<_>>(); // 1 x 6 = 6

    // @step When the pipeline packs the groups into sub-batches
    let subs = pack_groups(vec![vec![], group_a.clone(), group_b.clone(), group_c.clone()], 100, 8);

    // @step Then all three record groups land in a single sub-batch and every record's items stay together (empty groups are dropped)
    assert_eq!(subs.len(), 1, "empty group dropped, all three fit in one sub-batch");
    assert_eq!(subs[0].len(), 3, "one group entry per record");
    // groups are sorted by their max item length (deterministic): b(5), c(6), a(20)
    assert!(same_items(&subs[0][0], &group_b), "shortest group first");
    assert!(same_items(&subs[0][1], &group_c));
    assert!(same_items(&subs[0][2], &group_a));

    // @step And shrinking the token budget so one record group alone exceeds it splits only that record's items into ceil-sized chunks while the other records' items each stay intact in their own sub-batch
    // group a alone costs 20*2 = 40 > 30 -> split alone; b (5) and c (6) still fit.
    let subs2 = pack_groups(vec![group_a.clone(), group_b.clone(), group_c.clone()], 30, 8);
    let mut a_chunks = Vec::new();
    for sub in &subs2 {
        for g in sub {
            if g.iter().all(|i| i.rec_uid == 1) {
                a_chunks.push(g.clone());
            }
        }
    }
    assert_eq!(a_chunks.len(), 2, "oversized record splits into ceil(40/30) = 2 chunks");
    assert_eq!(a_chunks.iter().map(|g| g.len()).sum::<usize>(), group_a.len());
    // the other records' items each stay intact
    let intact: Vec<&Vec<Item>> = subs2
        .iter()
        .flat_map(|sub| sub.iter())
        .filter(|g| g.iter().all(|i| i.rec_uid == 2 || i.rec_uid == 3))
        .collect();
    assert!(intact.iter().any(|g| same_items(g, &group_b)), "group b intact");
    assert!(intact.iter().any(|g| same_items(g, &group_c)), "group c intact");
}

/// Scenario: Token batches respect the padded-token budget
#[test]
fn token_batches_respect_the_padded_token_budget() {
    // @step Given eight records have lengths [300, 300, 200, 200, 100, 100, 100, 100] with one sequence each
    let lengths = [300usize, 300, 200, 200, 100, 100, 100, 100];
    let nseq = [1usize; 8];
    let mut rng = StdRng::seed_from_u64(1234);

    // @step When the pipeline builds length-bucketed token batches under a 1000-token, 8-sequence budget
    let batches = make_token_batches(&lengths, &nseq, 1000, 8, &mut rng, 8);

    // @step Then every batch stays within the padded-token budget, every record appears in exactly one batch, and the batch order is shuffled
    let mut all = Vec::new();
    for b in &batches {
        let max_len = b.iter().map(|&i| lengths[i]).max().unwrap_or(0);
        let n = b.iter().map(|&i| nseq[i]).sum::<usize>();
        assert!(
            max_len * n <= 1000,
            "batch {b:?} padded cost {max_len} x {n} exceeds the 1000-token budget"
        );
        assert!(n <= 8, "batch {b:?} exceeds the 8-sequence budget");
        all.extend_from_slice(b);
    }
    all.sort_unstable();
    assert_eq!(
        all,
        (0..8).collect::<Vec<_>>(),
        "every record appears in exactly one batch"
    );
    // deterministic packing (length-sorted chunk): the greedy filler packs
    // [100,100,100,100,200] (padded 1000) and [200,300,300] (padded 900) —
    // one 5-record and one 3-record batch; the batch order itself is shuffled
    // by the caller RNG.
    let mut sizes: Vec<usize> = batches.iter().map(|b| b.len()).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, vec![3, 5], "one 5-record and one 3-record batch");
}

/// Scenario: Episode records yield one item per sampled prefix
#[test]
fn episode_records_yield_one_item_per_sampled_prefix() {
    // @step Given an episode record has a 3-turn conversation, outcome y=0.8, and the checkpoint allows 6 prefixes
    // Serialized state for prefix t: compact JSON of ctx + conversation = turns[..t]
    // (serde_json uses BTreeMap, so the key order is `conversation`, `topic`).
    let turns: Vec<serde_json::Value> = ["hello", "world", "done"]
        .iter()
        .map(|s| serde_json::Value::String(s.to_string()))
        .collect();
    let state_strs: Vec<String> = (1..=3)
        .map(|t| serde_json::to_string(&serde_json::json!({
            "conversation": turns[..t],
            "topic": "churn"
        }))
        .unwrap())
        .collect();
    let tok = test_tokenizer(&[
        "noul question: Does the user escalate?",
        "false: no, the statement does not hold",
        "true: yes, the statement holds",
        &state_strs[0],
        &state_strs[1],
        &state_strs[2],
    ]);
    let rec: Record = serde_json::from_str(
        r#"{"kind": "episode",
            "ep": {"ctx": {"topic": "churn"}, "turns": ["hello", "world", "done"], "y": 0.8},
            "qs": [{"type": "noul", "instructions": "Does the user escalate?"}]}"#,
    )
    .expect("parse episode record");
    let state_ids = |it: &Item, s: &str| {
        let id = tok.token_to_id(s).expect("state in vocab");
        it.ids.contains(&id)
    };

    // @step When the pipeline encodes the episode record
    let items = encode_record(&tok, &special(), &rec, 512, 512, 6, 7, None::<&mut StdRng>, true);

    // @step Then exactly 3 noul items are returned, one per prefix length 1..3, each with target [0.2, 0.8], label 1, and ep_len 3
    assert_eq!(items.len(), 3, "one item per prefix 1..3");
    let y = 0.8f32;
    for (step, it) in items.iter().enumerate() {
        assert_eq!(it.qtype, QType::Noul, "episode items are noul questions");
        assert_eq!(it.markers.len(), 2, "noul has exactly two options");
        for &m in &it.markers {
            assert_eq!(it.ids[m], MASK, "marker must point at a [MASK] token");
        }
        // target is [1-y, y]; compare approximately (1.0 - 0.8f32 is not bit-exact 0.2f32)
        assert!(
            (it.target[0] - (1.0 - y)).abs() < 1e-6 && (it.target[1] - y).abs() < 1e-6,
            "target must be [1-y, y]; got {:?}",
            it.target
        );
        assert_eq!(it.label, 1, "label is round(y) = round(0.8) = 1");
        assert!(it.episode, "episode flag set");
        assert_eq!(it.ep_step, step);
        assert_eq!(it.ep_len, 3);
        assert_eq!(it.rec_uid, 7);
    }
    // item k embeds the first k turns: prefix 1's state contains only turn 1,
    // prefix 2 adds turn 2, prefix 3 adds turn 3.
    assert!(state_ids(&items[0], &state_strs[0]), "prefix 1 state");
    assert!(!state_ids(&items[0], &state_strs[1]), "prefix 1 must not carry turn 2");
    assert!(state_ids(&items[1], &state_strs[1]), "prefix 2 state");
    assert!(!state_ids(&items[1], &state_strs[2]), "prefix 2 must not carry turn 3");
    assert!(state_ids(&items[2], &state_strs[2]), "prefix 3 state");

    // @step And restricting the episode to 2 sampled prefixes yields exactly 2 items at the evenly spaced prefix lengths 1 and 3
    let two = encode_record(&tok, &special(), &rec, 512, 512, 2, 7, None::<&mut StdRng>, true);
    assert_eq!(two.len(), 2, "max_prefixes=2 -> exactly 2 items at lengths 1 and 3");
    for it in &two {
        assert_eq!(it.ep_len, 2);
    }
    assert!(state_ids(&two[0], &state_strs[0]), "first sampled prefix is length 1");
    assert!(!state_ids(&two[0], &state_strs[1]), "first sampled prefix is not length 2");
    assert!(state_ids(&two[1], &state_strs[2]), "second sampled prefix is length 3");
}

/// Scenario: Episode targets are bootstrapped with TD lambda
#[test]
fn episode_targets_are_bootstrapped_with_td_lambda() {
    fn ep_item(uid: i64, step: usize) -> Item {
        Item {
            ids: vec![],
            markers: vec![],
            qtype: QType::Noul,
            target: vec![0.2, 0.8], // raw outcome target [1-y, y]
            label: 1,
            episode: true,
            ep_step: step,
            ep_len: 3,
            rec_uid: uid,
        }
    }

    // @step Given a batch has three episode prefix items of one record (raw outcome target [0.2, 0.8]) plus one non-episode item, and the policy's P(true) per item is [0.2, 0.5, 0.9]
    let items = vec![
        ep_item(1, 0),
        ep_item(1, 1),
        ep_item(1, 2),
        Item {
            ids: vec![],
            markers: vec![],
            qtype: QType::Choice,
            target: vec![0.6, 0.4],
            label: 0,
            episode: false,
            ep_step: 0,
            ep_len: 1,
            rec_uid: 2,
        },
    ];
    let p_true = [0.2f32, 0.5, 0.9, 0.4]; // 0.4 for the non-episode item (unused)

    // @step When the pipeline computes TD(lambda) targets for the batch with lambda 0.5
    let targets = td_lambda_targets(&items, &p_true, 0.5);

    // @step Then the episode items' targets become [0.675, 0.85, 0.8], walking backward from the final prefix and blending the policy's next-step P(true) with the previous target, while the non-episode item's target is left untouched
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    // g_2 = 0.8; g_1 = 0.5*0.9 + 0.5*0.8 = 0.85; g_0 = 0.5*0.5 + 0.5*0.85 = 0.675
    assert!(close(targets[0][1], 0.675), "first prefix target: {:?}", targets[0]);
    assert!(close(targets[0][0], 1.0 - 0.675));
    assert!(close(targets[1][1], 0.85), "second prefix target: {:?}", targets[1]);
    assert!(close(targets[1][0], 1.0 - 0.85));
    assert!(close(targets[2][1], 0.8), "final prefix keeps the raw outcome target");
    assert!(close(targets[2][0], 0.2));
    assert_eq!(targets[3], [0.6, 0.4], "non-episode item is left untouched");
}
