//! Goal: "You have hexproof." and "You have shroud." read into a static ability that gives the keyword to the controller, with a leading condition when there is one, write back as the card's own text, and the keywords a player cannot have stay with the table.
use super::*;

#[rstest]
#[case::hexproof(
    "You have hexproof.",
    r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"}]}}"#
)]
#[case::shroud(
    "You have shroud.",
    r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Shroud"}]}}"#
)]
#[case::both(
    "You have hexproof and shroud.",
    r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"},{"AddKeyword":"Shroud"}]}}"#
)]
#[case::with_the_commander(
    "As long as you control your commander, you have hexproof.",
    r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"}],"condition":{"Compare":{"left":{"Count":[{"Commander":null},{"OwnedBy":"You"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#
)]
#[case::after_the_ability_word(
    "Lieutenant \u{2014} As long as you control your commander, you have hexproof.",
    r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"}],"condition":{"Compare":{"left":{"Count":[{"Commander":null},{"OwnedBy":"You"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#
)]
fn a_player_keyword_reads_into_a_static_ability_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    let without = text.split(" \u{2014} ").last().unwrap_or(text);
    reads_as(without, ClauseKind::Static, false, json);
    let read = clause(text, ClauseKind::Static, false);
    assert_eq!(read, Some(ability(json)), "{text}");
}

#[rstest]
#[case::a_keyword_a_player_cannot_have("You have flying.")]
#[case::a_keyword_with_a_pump("You have hexproof and get +1/+1.")]
#[case::an_opponent("Your opponents have hexproof.")]
#[case::a_condition_after("You have hexproof as long as you control your commander.")]
#[case::lifelink_with_hexproof("You have hexproof and lifelink.")]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
