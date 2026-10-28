//! Goal: "you may pay X life, where X is ... If you do, ..." reads as a `Pay` of the counted life whose paid effects use the same count in the place of X, writes back as the card's text, and a payment of an X the payer chooses or with no count stays with the table.
use super::*;
use crate::cost::CostItem;
use crate::effect::Effect;

#[rstest]
#[case::tivash(
    "At the beginning of your end step, if you gained life this turn, you may pay X life, where X is the amount of life you gained this turn. If you do, create an X/X black Demon creature token with flying."
)]
#[case::mask(
    "Whenever equipped creature dies, you may pay X life, where X is its power. If you do, draw X cards."
)]
fn c10_pa_a_counted_life_payment_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let [Effect::Pay { cost, paid, .. }] = read.effects() else {
        panic!("not one payment: {read:?}");
    };
    let [CostItem::PayLife(amount)] = cost.0.as_slice() else {
        panic!("not a life payment: {cost:?}");
    };
    assert!(!matches!(amount, crate::value::Amount::X), "{text}");
    let json = serde_json::to_string(paid).unwrap();
    assert!(!json.contains("\"X\""), "{text}: {json}");
    assert!(
        json.contains(&serde_json::to_string(amount).unwrap()),
        "{text}: {json}"
    );
}

#[rstest]
#[case::no_where("Whenever equipped creature dies, you may pay X life. If you do, draw X cards.")]
#[case::fixed(
    "Whenever equipped creature dies, you may pay X life, where X is 3. If you do, draw X cards."
)]
#[case::no_if_you_do(
    "Whenever equipped creature dies, you may pay X life, where X is its power. Draw X cards."
)]
fn c10_pa_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}
