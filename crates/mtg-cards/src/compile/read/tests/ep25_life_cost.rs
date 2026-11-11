//! Goal: the writer of a cost increase in life gives "Spells your opponents cast that target ~ cost an additional 3 life to cast.", and another amount, caster, change, a cost with mana beside the life and a mana wording never read back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const TERROR: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":["This"]}],"change":"Increase","mana":"{0}","life":3}}"#;
const WITH_MANA: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":["This"]}],"change":"Increase","mana":"{1}","life":3}}"#;
const LESS: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"Targeting":["This"]}],"change":"Reduce","mana":"{0}","life":3}}"#;

#[rstest]
#[case::terror(
    TERROR,
    "Spells your opponents cast that target ~ cost an additional 3 life to cast.",
    true
)]
#[case::another_amount(
    TERROR,
    "Spells your opponents cast that target ~ cost an additional 2 life to cast.",
    false
)]
#[case::your_own(
    TERROR,
    "Spells you cast that target ~ cost an additional 3 life to cast.",
    false
)]
#[case::as_mana(
    TERROR,
    "Spells your opponents cast that target ~ cost {3} more to cast.",
    false
)]
#[case::mana_beside(
    WITH_MANA,
    "Spells your opponents cast that target ~ cost an additional 3 life to cast.",
    false
)]
#[case::mana_beside_as_mana(
    WITH_MANA,
    "Spells your opponents cast that target ~ cost {1} more to cast.",
    false
)]
#[case::a_reduction(
    LESS,
    "Spells your opponents cast that target ~ cost an additional 3 life to cast.",
    false
)]
fn ep25_a_cost_in_life_reads_back(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{clause}");
}
