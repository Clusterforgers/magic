//! Goal: the round 13 trigger events write back as the card words them ("Whenever an opponent searches their library",
//! "Whenever a creature is exiled from the battlefield", "Whenever you play a land from exile", "Whenever you clash and
//! win") and a rule that means something else (any library, any exile, a land from anywhere, a clash lost by another
//! player) does not write those words.
use super::*;

/// Whether the ability writes the clause back, among its wordings.
fn writes(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

/// A triggered ability of the event that draws a card, or may draw one when the clause says so.
fn draws(when: &str, clause: &str) -> String {
    let draw = r#"{"Draw":{}}"#;
    let effect = if clause.contains("you may draw") {
        format!(r#"{{"May":{{"effects":[{draw}]}}}}"#)
    } else {
        draw.to_owned()
    };
    format!(r#"{{"Triggered":{{"when":{when},"effects":[{effect}]}}}}"#)
}

#[rstest]
#[case::opponent_searches(
    r#"{"Did":{"action":"SearchedOwnLibrary","who":"Opponents"}}"#,
    "Whenever an opponent searches their library, draw a card.",
    true
)]
#[case::you_search(
    r#"{"Did":{"action":"SearchedOwnLibrary","who":"You"}}"#,
    "Whenever you search your library, draw a card.",
    true
)]
// Any library is not "their library".
#[case::any_library(
    r#"{"Did":{"action":"SearchedLibrary","who":"Opponents"}}"#,
    "Whenever an opponent searches their library, draw a card.",
    false
)]
#[case::wrong_player(
    r#"{"Did":{"action":"SearchedOwnLibrary","who":"You"}}"#,
    "Whenever an opponent searches their library, draw a card.",
    false
)]
#[case::exiled_from_the_battlefield(
    r#"{"Did":{"action":"Exiled","what":[{"Type":"Creature"},{"In":"Battlefield"}]}}"#,
    "Whenever a creature is exiled from the battlefield, draw a card.",
    true
)]
#[case::another_exiled(
    r#"{"Did":{"action":"Exiled","what":[{"Type":"Creature"},"Other",{"In":"Battlefield"}]}}"#,
    "Whenever another creature is exiled from the battlefield, draw a card.",
    true
)]
// An exile from anywhere is not "from the battlefield".
#[case::exiled_anywhere(
    r#"{"Did":{"action":"Exiled","what":[{"Type":"Creature"}]}}"#,
    "Whenever a creature is exiled from the battlefield, draw a card.",
    false
)]
#[case::land_from_exile(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"},{"ArrivedFrom":"Exile"}]}}"#,
    "Whenever you play a land from exile, draw a card.",
    true
)]
#[case::land_from_anywhere(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"You"}]}}"#,
    "Whenever you play a land from exile, draw a card.",
    false
)]
#[case::opponents_land_from_exile(
    r#"{"Did":{"action":"PlayedLand","what":[{"Type":"Land"},{"ControlledBy":"Opponents"},{"ArrivedFrom":"Exile"}]}}"#,
    "Whenever you play a land from exile, draw a card.",
    false
)]
#[case::clash_and_win(
    r#"{"W15cClashWon":{"who":"You"}}"#,
    "Whenever you clash and win, you may draw a card.",
    true
)]
#[case::opponent_wins(
    r#"{"W15cClashWon":{"who":"Opponents"}}"#,
    "Whenever you clash and win, you may draw a card.",
    false
)]
fn g13c_a_trigger_event_writes_its_words(
    #[case] when: &str,
    #[case] clause: &str,
    #[case] written: bool,
) {
    assert_eq!(writes(&draws(when, clause), clause), written, "{clause}");
}
