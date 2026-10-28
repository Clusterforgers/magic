//! Goal: a cost change with a condition ("if you control a Dragon", "if there are ten or more nonland permanents on the battlefield", "If there are ten or more creature cards total in all graveyards, ...", "if an opponent controls no basic lands", "if you gained life this turn") and one for spells of two kinds ("Merfolk spells and Wizard spells you cast", "Aura and Equipment spells you cast") read into a `CostModifier`, write back as the same text, and leave every near-miss to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

/// A reduction of the card itself under `condition`.
fn itself(mana: &str, condition: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{mana}","condition":{condition}}}}}"#
    )
}

#[rstest]
#[case::control_a_dragon(
    "This spell costs {2} less to cast if you control a Dragon.",
    "{2}",
    r#"{"Compare":{"left":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
#[case::control_a_creature_with_flying(
    "This spell costs {1} less to cast if you control a creature with flying.",
    "{1}",
    r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
#[case::nonland_permanents_on_the_battlefield(
    "This spell costs {3} less to cast if there are ten or more nonland permanents on the battlefield.",
    "{3}",
    r#"{"Compare":{"left":{"Count":[{"Not":{"Type":"Land"}}]},"op":"Ge","right":10}}"#
)]
#[case::creature_cards_in_all_graveyards(
    "If there are ten or more creature cards total in all graveyards, this spell costs {6} less to cast.",
    "{6}",
    r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]},"op":"Ge","right":10}}"#
)]
#[case::an_opponent_controls_no_basic_lands(
    "This spell costs {1} less to cast if an opponent controls no basic lands.",
    "{1}",
    r#"{"Compare":{"left":{"Count":[{"Type":"Land"},{"Supertype":"Basic"},{"ControlledBy":"Opponents"}]},"op":"Eq","right":0}}"#
)]
#[case::life_gained_this_turn(
    "This spell costs {2} less to cast if you gained life this turn.",
    "{2}",
    r#"{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGained"}},"op":"Ge","right":1}}"#
)]
fn a_reduction_under_a_condition(#[case] text: &str, #[case] mana: &str, #[case] condition: &str) {
    reads_as(text, ClauseKind::Static, false, &itself(mana, condition));
}

#[rstest]
#[case::subtypes(
    "Merfolk spells and Wizard spells you cast cost {1} less to cast.",
    r#"["Spell",{"AnyOf":[{"Subtype":"Merfolk"},{"Subtype":"Wizard"}]},{"ControlledBy":"You"}]"#
)]
#[case::colors(
    "Blue spells and red spells you cast cost {1} less to cast.",
    r#"["Spell",{"AnyOf":[{"Color":"Blue"},{"Color":"Red"}]},{"ControlledBy":"You"}]"#
)]
fn a_reduction_for_spells_of_two_kinds(#[case] text: &str, #[case] applies_to: &str) {
    let json = format!(
        r#"{{"CostModifier":{{"applies_to":{applies_to},"change":"Reduce","mana":"{{1}}"}}}}"#
    );
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::one_opponent_with_cards(
    "This spell costs {3} less to cast if an opponent has seven or more cards in their graveyard."
)]
#[case::a_condition_unread("This spell costs {1} less to cast if the moon is full.")]
#[case::the_zone_of_the_count_named(
    "This spell costs {3} less to cast if there are ten or more creatures you control on the battlefield."
)]
fn a_condition_the_reader_does_not_know_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
