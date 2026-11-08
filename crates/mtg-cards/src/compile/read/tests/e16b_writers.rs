//! Goal: an optional blight cost of a spell reads back as "you may blight N" and its payment as "if this spell's additional
//! cost was paid"; another count, a required cost or the condition turned around never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const BLIGHT_ONE: &str = r#"{"AdditionalCost":{"cost":[{"Blight":1}]}}"#;
const GAIN_IF_PAID: &str = r#"{"Spell":{"effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"GainLife":{"amount":2}}]}}]}}"#;
const GAIN_IF_NOT_PAID: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Not":"AdditionalCostPaid"},"then":[{"GainLife":{"amount":2}}]}}]}}"#;

#[rstest]
#[case::blight_one(
    BLIGHT_ONE,
    "As an additional cost to cast this spell, you may blight 1."
)]
#[case::blight_two(
    r#"{"AdditionalCost":{"cost":[{"Blight":2}]}}"#,
    "As an additional cost to cast this spell, you may blight 2."
)]
#[case::required_blight(
    r#"{"AdditionalCost":{"cost":[{"Blight":1}],"required":true}}"#,
    "As an additional cost to cast this spell, blight 1."
)]
#[case::paid(
    GAIN_IF_PAID,
    "If this spell's additional cost was paid, you gain 2 life."
)]
fn the_optional_cost_and_its_payment_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_count(
    BLIGHT_ONE,
    "As an additional cost to cast this spell, you may blight 2."
)]
#[case::may_left_out(BLIGHT_ONE, "As an additional cost to cast this spell, blight 1.")]
#[case::may_added(
    r#"{"AdditionalCost":{"cost":[{"Blight":1}],"required":true}}"#,
    "As an additional cost to cast this spell, you may blight 1."
)]
#[case::not_paid(
    GAIN_IF_NOT_PAID,
    "If this spell's additional cost was paid, you gain 2 life."
)]
#[case::another_amount(
    GAIN_IF_PAID,
    "If this spell's additional cost was paid, you gain 3 life."
)]
fn another_cost_or_condition_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
