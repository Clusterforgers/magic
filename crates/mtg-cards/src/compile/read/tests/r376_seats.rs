//! Goal: a player named by their seat ("the player to your right", "the opponent to your left") reads as `Ref::Seated`
//! and writes back, "the player to your right gains control of ~" and "Reverse the game's turn order." read and write
//! back, and a near miss of the text or of the rules reads or writes as something else.
use super::*;

fn seated_control(side: &str, opponent: bool) -> String {
    let opponent = if opponent { r#","opponent":true"# } else { "" };
    format!(
        r#"{{"Spell":{{"effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetController":{{"Seated":{{"of":"You","side":"{side}"{opponent}}}}}}}],"duration":"Forever"}}}}]}}}}"#
    )
}

#[rstest]
#[case::right("The player to your right gains control of ~.", "Right", false)]
#[case::left("The player to your left gains control of ~.", "Left", false)]
#[case::opponent("The opponent to your left gains control of ~.", "Left", true)]
fn the_neighbour_gains_control_of_the_card(
    #[case] text: &str,
    #[case] side: &str,
    #[case] opponent: bool,
) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &seated_control(side, opponent),
    );
}

#[test]
fn the_neighbour_draws() {
    reads_as(
        "The player to your right draws a card.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"effects":[{"Draw":{"who":{"Seated":{"of":"You","side":"Right"}}}}]}}"#,
    );
}

#[test]
fn the_turn_order_is_reversed_for_good() {
    reads_as(
        "Reverse the game's turn order.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"effects":[{"Rule":{"rule":"ReverseTurnOrder","duration":"Forever"}}]}}"#,
    );
}

#[rstest]
#[case::a_player("A player to your right gains control of ~.")]
#[case::their_right("The player to their right gains control of ~.")]
#[case::up("The player to your up gains control of ~.")]
#[case::the_turn("Reverse the turn order.")]
fn a_near_miss_of_the_text_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::left_for_right(&seated_control("Left", false), "The player to your right gains control of ~.")]
#[case::opponent_for_player(&seated_control("Right", true), "The player to your right gains control of ~.")]
#[case::for_the_turn(
    &seated_control("Right", false).replace("Forever", "UntilEndOfTurn"),
    "The player to your right gains control of ~."
)]
#[case::of_each_player(
    &seated_control("Right", false).replace(r#""of":"You""#, r#""of":"EachPlayer""#),
    "The player to your right gains control of ~."
)]
#[case::reverse_for_the_turn(
    r#"{"Spell":{"effects":[{"Rule":{"rule":"ReverseTurnOrder","duration":"UntilEndOfTurn"}}]}}"#,
    "Reverse the game's turn order."
)]
fn a_near_miss_of_the_rules_is_not_written_as_the_text(#[case] json: &str, #[case] text: &str) {
    let written = render_variants(&ability(json));
    assert!(
        !written.iter().any(|said| same_text(said, text)),
        "{json}: {written:?}"
    );
}

#[rstest]
#[case::d4(4)]
#[case::d20(20)]
fn a_roll_for_treasures_reads_before_another_sentence(#[case] sides: u32) {
    let text = format!(
        "Roll a d{sides} and create a number of Treasure tokens equal to the result. The player to your right gains control of ~."
    );
    reads_as(
        &text,
        ClauseKind::Spell,
        false,
        &format!(
            r#"{{"Spell":{{"effects":[{{"RollDice":{{"sides":{sides},"results":[{{"from":1,"effects":[{{"CreateToken":{{"token":{{"Named":"treasure"}},"count":{{"DieResult":"Only"}}}}}}]}}]}}}},{{"Continuous":{{"affected":"This","mods":[{{"SetController":{{"Seated":{{"of":"You","side":"Right"}}}}}}],"duration":"Forever"}}}}]}}}}"#
        ),
    );
}

#[rstest]
#[case::clues("Roll a d4 and create a number of Clue tokens equal to the result.")]
#[case::half("Roll a d4 and create half a number of Treasure tokens equal to the result.")]
fn a_near_miss_of_the_roll_is_not_read_as_treasures(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    assert!(
        !format!("{read:?}").contains(r#"Named("treasure")"#),
        "{text}: {read:?}"
    );
}

#[rstest]
#[case::itself(
    r#"{"Rule":{"rule":{"Cant":{"who":"This","action":"Sacrifice"}}}}"#,
    true
)]
#[case::each_player(
    r#"{"Rule":{"rule":{"Cant":{"who":"EachPlayer","action":"Sacrifice"}}}}"#,
    false
)]
#[case::countered(
    r#"{"Rule":{"rule":{"Cant":{"who":"This","action":"PutCounter"}}}}"#,
    false
)]
fn the_card_that_cant_be_sacrificed_is_written_so(#[case] json: &str, #[case] written: bool) {
    let said = crate::compile::read::render::written_variants(&ability(json));
    assert_eq!(
        said.iter()
            .any(|text| same_text(text, "This artifact can't be sacrificed.")),
        written,
        "{json}: {said:?}"
    );
}
