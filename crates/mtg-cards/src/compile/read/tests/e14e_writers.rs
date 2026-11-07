//! Goal: the writer of round 14 pack e14e gives the oracle wording of a cost change for the equip abilities that target the source, and another amount, direction, target, controller or kind of ability never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CHAMPION: &str = r#"{"CostModifier":{"activating":"Equip","applies_to":[{"ControlledBy":"You"}],"change":"Reduce","mana":"{3}","targeting":["This"]}}"#;
const GOLEM: &str = r#"{"CostModifier":{"activating":"Equip","applies_to":[{"ControlledBy":"You"}],"change":"Increase","mana":"{2}","targeting":["This"]}}"#;
const ANY_KIND: &str = r#"{"CostModifier":{"activating":"Any","applies_to":[{"ControlledBy":"You"}],"change":"Reduce","mana":"{3}","targeting":["This"]}}"#;
const THEIRS: &str = r#"{"CostModifier":{"activating":"Equip","applies_to":[{"ControlledBy":"Opponents"}],"change":"Reduce","mana":"{3}","targeting":["This"]}}"#;
const ANOTHER: &str = r#"{"CostModifier":{"activating":"Equip","applies_to":[{"ControlledBy":"You"}],"change":"Reduce","mana":"{3}","targeting":[{"Type":"Creature"}]}}"#;

#[rstest]
#[case::less(
    CHAMPION,
    "Equip abilities you activate that target ~ cost {3} less to activate."
)]
#[case::more(
    GOLEM,
    "Equip abilities you activate that target ~ cost {2} more to activate."
)]
fn the_writer_reads_the_clause(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{json}");
}

#[rstest]
#[case::another_amount(
    CHAMPION,
    "Equip abilities you activate that target ~ cost {2} less to activate."
)]
#[case::the_other_direction(
    CHAMPION,
    "Equip abilities you activate that target ~ cost {3} more to activate."
)]
#[case::every_ability(
    ANY_KIND,
    "Equip abilities you activate that target ~ cost {3} less to activate."
)]
#[case::opponents_abilities(
    THEIRS,
    "Equip abilities you activate that target ~ cost {3} less to activate."
)]
#[case::another_target(
    ANOTHER,
    "Equip abilities you activate that target ~ cost {3} less to activate."
)]
fn a_wrong_rule_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{json}");
}
