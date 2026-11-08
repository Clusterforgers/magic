//! Goal: each capability the engine lists is named by the rule that uses it, and the games that fire it live in `did_triggers` and `opening_hand`; the untap symbol is paid (its games live in `r5g`).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::validate::used_primitives;
use rstest::rstest;

#[rstest]
#[case::a_land_played(
    r#"{"Triggered":{"when":{"Did":{"action":"PlayedLand"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    "Trigger.Did",
    true
)]
#[case::a_scry(
    r#"{"Triggered":{"when":{"Did":{"action":"Scried","who":"You"}},"effects":[{"Draw":{}}]}}"#,
    "Trigger.Did",
    true
)]
#[case::the_opening_hand(r#""OpeningHand""#, "Ability.OpeningHand", true)]
#[case::untapping_the_source_as_a_cost(
    r#"{"Activated":{"cost":["Untap"],"effects":[{"Draw":{}}]}}"#,
    "Cost.Untap",
    true
)]
fn a_listed_capability_is_named_by_its_rule(
    #[case] rule: &str,
    #[case] name: &str,
    #[case] runs: bool,
) {
    let ability: Ability = serde_json::from_str(rule).unwrap();
    assert!(used_primitives(&ability).contains(name));
    assert_eq!(mtg_engine::caps().runs(name), runs);
}
