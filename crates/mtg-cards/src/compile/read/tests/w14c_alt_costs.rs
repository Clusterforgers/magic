//! Goal: "You may <cost> rather than pay this spell's mana cost.", with or without a condition on what the caster controls, reads as a cast for that cost in place of the mana cost (mana, life, a sacrifice, a return to the hand) and is written back as the same text; a cost with an X, another player or another verb stays with the table.
use super::*;

const SWAMP: &str = r#"{"Compare":{"left":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

#[rstest]
#[case::a_sacrifice(
    "You may sacrifice a Mountain rather than pay this spell's mana cost.",
    None,
    r#"[{"Sacrifice":{"filter":[{"Subtype":"Mountain"}]}}]"#
)]
#[case::life_with_a_condition(
    "If you control a Swamp, you may pay 4 life rather than pay this spell's mana cost.",
    Some(SWAMP),
    r#"[{"PayLife":4}]"#
)]
#[case::mana(
    "You may pay {1}{B} rather than pay this spell's mana cost.",
    None,
    r#"[{"Mana":"{1}{B}"}]"#
)]
#[case::mana_with_a_condition(
    "If you control a Swamp, you may pay {B} rather than pay this spell's mana cost.",
    Some(SWAMP),
    r#"[{"Mana":"{B}"}]"#
)]
fn w14c_an_alternative_cost_reads_and_is_written_back(
    #[case] text: &str,
    #[case] condition: Option<&str>,
    #[case] cost: &str,
) {
    let condition = condition.map_or(String::new(), |held| format!(r#""condition":{held},"#));
    let json = format!(r#"{{"FreeCast":{{{condition}"cost":{cost}}}}}"#);
    reads_as(text, ClauseKind::Static, false, &json);
}

#[rstest]
#[case::an_x("You may pay {X} rather than pay this spell's mana cost.")]
#[case::another_verb("You may sacrifice a Mountain rather than cast this spell.")]
#[case::another_player(
    "If an opponent controls a Swamp, you may pay 4 life rather than pay this spell's mana cost."
)]
#[case::no_cost("You may rather than pay this spell's mana cost.")]
fn w14c_near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
