//! Goal: the wordings of the prepare cards ("This creature enters prepared.", "~ becomes prepared" after the events and conditions the cards print) read into the DSL, write back as the card's own text, and leave the near-misses (another subject, another state, a target, a "may") to the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[test]
fn a_prepare_card_enters_prepared() {
    reads_as(
        "~ enters prepared.",
        ClauseKind::Static,
        false,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":"EntersPrepared"}}"#,
    );
}

#[rstest]
#[case::a_land("~ enters prepared and tapped.")]
#[case::another("Another creature enters prepared.")]
#[case::unprepared("~ enters unprepared.")]
#[case::in_the_middle("~ enters the battlefield prepared.")]
fn near_misses_of_entering_prepared_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}

#[rstest]
#[case::upkeep(
    "At the beginning of your upkeep, ~ becomes prepared.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
#[case::an_end_step_if_life_was_gained(
    "At the beginning of each end step, if you gained life this turn, ~ becomes prepared.",
    r#"{"Triggered":{"if":{"Compare":{"op":"Ge","left":{"Tallied":{"who":"You","what":"LifeGained"}},"right":1}},"when":{"StepBegins":{"step":"End"}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
#[case::attacking_with_two(
    "Whenever you attack with two or more creatures, ~ becomes prepared.",
    r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
#[case::an_opponents_second_draw(
    "Whenever an opponent draws their second card each turn, ~ becomes prepared.",
    r#"{"Triggered":{"when":{"Nth":{"nth":2,"event":{"Drew":{"who":"Opponents"}}}},"effects":[{"Prepare":{"what":"This"}}]}}"#
)]
fn a_trigger_prepares_the_card(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::another_creature("Whenever you attack, another creature becomes prepared.")]
#[case::unprepared("Whenever you attack, ~ becomes unprepared.")]
#[case::a_target("Whenever you attack, target creature becomes prepared.")]
#[case::a_may("Whenever you attack, you may have ~ become prepared.")]
#[case::every_creature("Whenever you attack, each creature you control becomes prepared.")]
#[case::all_prepared("Whenever you attack, ~ and each other creature become prepared.")]
fn near_misses_of_becoming_prepared_stay_with_the_table(#[case] text: &str) {
    assert_eq!(trigger(text), None);
}
