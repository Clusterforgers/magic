//! Goal: round 12 pack g12f writes the tax of the X paid for an ability, for this turn, as War Cadence and War Tax print it; another price, payer, creature, duration or count does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn activated(cost: &str, rule: &str, duration: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{cost}","effects":[{{"Rule":{{"duration":"{duration}","rule":{rule}}}}}]}}}}"#
    )
}

const CADENCE: &str = "{X}{R}: This turn, creatures can't block unless their controller pays {X} for each blocking creature they control.";
const TAX: &str = "{X}{U}: This turn, creatures can't attack unless their controller pays {X} for each attacking creature they control.";
const BLOCK_X: &str = r#"{"BlockTax":{"mana":"{1}","per":"X"}}"#;
const ATTACK_X: &str =
    r#"{"AttackTax":{"attacked":"Both","mana":"{1}","per":"X","who":"EachPlayer"}}"#;

#[rstest]
#[case::war_cadence(activated("{X}{R}", BLOCK_X, "UntilEndOfTurn"), CADENCE, true)]
#[case::cadence_a_fixed_price(
    activated("{X}{R}", r#"{"BlockTax":{"mana":"{1}"}}"#, "UntilEndOfTurn"),
    CADENCE,
    false
)]
#[case::cadence_until_your_next_turn(
    activated("{X}{R}", BLOCK_X, "UntilYourNextTurn"),
    CADENCE,
    false
)]
#[case::cadence_only_some_blockers(
    activated(
        "{X}{R}",
        r#"{"BlockTax":{"blockers":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"mana":"{1}","per":"X"}}"#,
        "UntilEndOfTurn"
    ),
    CADENCE,
    false
)]
#[case::cadence_life(
    activated("{X}{R}", r#"{"BlockTax":{"life":1,"per":"X"}}"#, "UntilEndOfTurn"),
    CADENCE,
    false
)]
#[case::war_tax(activated("{X}{U}", ATTACK_X, "UntilEndOfTurn"), TAX, true)]
#[case::tax_only_attacks_on_you(
    activated(
        "{X}{U}",
        r#"{"AttackTax":{"attacked":"Player","mana":"{1}","per":"X"}}"#,
        "UntilEndOfTurn"
    ),
    TAX,
    false
)]
#[case::tax_two_each(
    activated(
        "{X}{U}",
        r#"{"AttackTax":{"attacked":"Both","mana":"{2}","per":"X","who":"EachPlayer"}}"#,
        "UntilEndOfTurn"
    ),
    TAX,
    false
)]
#[case::tax_read_as_a_block(activated("{X}{U}", BLOCK_X, "UntilEndOfTurn"), TAX, false)]
fn the_x_paid_taxes_this_turn(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}
