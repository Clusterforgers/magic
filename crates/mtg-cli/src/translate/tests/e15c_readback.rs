//! Goal: the read-back of round 15 pack e15c says "you may blight 1" for an optional blight, and "if this spell's
//! additional cost was paid", "if a Dragon was beheld" and "if evidence was collected" for the paid additional cost of a
//! spell whose one additional cost is that cost; another count, cost, subtype or a negated condition reads `Differs`.
use std::collections::BTreeSet;

use rstest::rstest;
use serde_json::{Value, json};

use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check_exact};
use mtg_cards::support::{PrintedCard, PrintedFace};

fn printed(oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-0000000e15c0".into(),
        name: "Alpha".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Alpha".into(),
            mana_cost: "{1}{B}".into(),
            type_line: "Sorcery".into(),
            oracle_text: oracle.into(),
            ..PrintedFace::default()
        }],
    }
}

fn verdict(oracle: &str, cost: &Value, condition: &Value) -> Verdict {
    let card = printed(oracle);
    let caps = mtg_engine::caps();
    let names = BTreeSet::from(["alpha".to_owned()]);
    let spell = json!({"Spell": {"effects": [
        {"Draw": {}},
        {"If": {"condition": condition, "then": [{"GainLife": {"amount": 2}}]}}
    ]}});
    let json =
        json!({"faces": [{"abilities": [spell, {"AdditionalCost": {"cost": cost}}]}]}).to_string();
    check_exact(&json, &card, &Library::default(), &names, &caps).verdict()
}

const BLIGHT: &str = "As an additional cost to cast this spell, you may blight 1.\nDraw a card. If this spell's additional cost was paid, you gain 2 life.";
const BEHELD: &str = "As an additional cost to cast this spell, you may behold a Dragon.\nDraw a card. If a Dragon was beheld, you gain 2 life.";
const EVIDENCE: &str = "As an additional cost to cast this spell, you may collect evidence 6.\nDraw a card. If evidence was collected, you gain 2 life.";

#[rstest]
#[case::blight(BLIGHT, json!([{"Blight": 1}]), json!("AdditionalCostPaid"), Verdict::Accepted)]
#[case::blight_two(BLIGHT, json!([{"Blight": 2}]), json!("AdditionalCostPaid"), Verdict::Mismatch)]
#[case::blight_not_paid(
    BLIGHT,
    json!([{"Blight": 1}]),
    json!({"Not": "AdditionalCostPaid"}),
    Verdict::Mismatch
)]
// The collect-evidence and plain behold costs have writers, so their cards read back whole.
#[case::evidence(EVIDENCE, json!([{"CollectEvidence": 6}]), json!("AdditionalCostPaid"), Verdict::Accepted)]
#[case::evidence_not_collected(
    EVIDENCE,
    json!([{"CollectEvidence": 6}]),
    json!({"Not": "AdditionalCostPaid"}),
    Verdict::Mismatch
)]
#[case::evidence_for_a_blight(EVIDENCE, json!([{"Blight": 1}]), json!("AdditionalCostPaid"), Verdict::Mismatch)]
#[case::beheld(
    BEHELD,
    json!([{"Behold": {"filter": [{"Subtype": "Dragon"}]}}]),
    json!("AdditionalCostPaid"),
    Verdict::Accepted
)]
#[case::beheld_goblin(
    BEHELD,
    json!([{"Behold": {"filter": [{"Subtype": "Goblin"}]}}]),
    json!("AdditionalCostPaid"),
    Verdict::Mismatch
)]
#[case::beheld_and_exiled(
    BEHELD,
    json!([{"Behold": {"filter": [{"Subtype": "Dragon"}], "exile": true}}]),
    json!("AdditionalCostPaid"),
    Verdict::Mismatch
)]
#[case::not_beheld(
    BEHELD,
    json!([{"Behold": {"filter": [{"Subtype": "Dragon"}]}}]),
    json!({"Not": "AdditionalCostPaid"}),
    Verdict::Mismatch
)]
fn the_paid_additional_cost_reads_back_in_the_words_of_its_cost(
    #[case] oracle: &str,
    #[case] cost: Value,
    #[case] condition: Value,
    #[case] expected: Verdict,
) {
    assert_eq!(verdict(oracle, &cost, &condition), expected);
}
