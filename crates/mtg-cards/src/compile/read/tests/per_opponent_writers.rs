//! Goal: a count taken for each opponent apart ("an opponent controls four or more lands") is written in a trigger's condition, an activation's limit and an "as long as" condition, in digits and in words, and a count over all opponents together, another number, group or player never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn at_least(group: &str, who: &str, number: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Largest":{{"who":{who},"of":[{group},{{"ControlledBy":"You"}}]}}}},"op":"Ge","right":{number}}}}}"#
    )
}

fn upkeep(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"if":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

fn activation(condition: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"condition":{condition},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

fn flying(condition: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"AddKeyword":"Flying"}}],"condition":{condition}}}}}"#
    )
}

const CREATURES: &str = r#"{"Type":"Creature"}"#;
const LANDS: &str = r#"{"Type":"Land"}"#;
const NONBASIC_LANDS: &str = r#"{"Type":"Land"},{"Not":{"Supertype":"Basic"}}"#;
const OPPONENTS: &str = r#""Opponents""#;

#[rstest]
#[case::a_trigger_in_words(upkeep(&at_least(CREATURES, OPPONENTS, 3)), "At the beginning of your upkeep, if an opponent controls three or more creatures, draw a card.")]
#[case::a_trigger_in_digits(upkeep(&at_least(CREATURES, OPPONENTS, 3)), "At the beginning of your upkeep, if an opponent controls 3 or more creatures, draw a card.")]
#[case::an_activation(activation(&at_least(LANDS, OPPONENTS, 4)), "{T}: Draw a card. Activate only if an opponent controls four or more lands.")]
#[case::a_static(flying(&at_least(CREATURES, OPPONENTS, 2)), "~ has flying as long as an opponent controls two or more creatures.")]
#[case::a_group_with_a_quality(activation(&at_least(NONBASIC_LANDS, OPPONENTS, 4)), "{T}: Draw a card. Activate only if an opponent controls four or more nonbasic lands.")]
fn a_count_for_each_opponent_apart_is_written_as_an_opponent_controls(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::all_opponents_together(
    "{T}: Draw a card. Activate only if your opponents control four or more lands."
)]
#[case::another_number(
    "{T}: Draw a card. Activate only if an opponent controls five or more lands."
)]
#[case::another_group(
    "{T}: Draw a card. Activate only if an opponent controls four or more creatures."
)]
#[case::you("{T}: Draw a card. Activate only if you control four or more lands.")]
#[case::fewer("{T}: Draw a card. Activate only if an opponent controls four or fewer lands.")]
#[case::exactly("{T}: Draw a card. Activate only if an opponent controls exactly four lands.")]
#[case::each_opponent(
    "{T}: Draw a card. Activate only if each opponent controls four or more lands."
)]
fn a_count_for_each_opponent_apart_does_not_read_as_another_count(#[case] clause: &str) {
    assert!(
        !written(&activation(&at_least(LANDS, OPPONENTS, 4)), clause),
        "{clause}"
    );
}

#[rstest]
#[case::the_player_is_you(at_least(LANDS, r#""You""#, 4))]
#[case::each_player(at_least(LANDS, r#""EachPlayer""#, 4))]
#[case::one_is_a_land(at_least(LANDS, OPPONENTS, 1))]
fn a_count_that_is_not_one_of_the_opponents_is_not_worded_as_an_opponent_controls(
    #[case] condition: String,
) {
    let json = activation(&condition);
    let wordings = written_variants(&ability(&json));
    assert!(
        wordings
            .iter()
            .all(|wording| !wording.contains("an opponent controls")),
        "{wordings:?}"
    );
}
