//! Goal: the keyword riot reads as a keyword line and as a keyword a static ability gives to a group, an activation restricted to a permanent with a counter on it reads and writes back, and a granted riot beside other text stays with the table.
use super::*;

#[rstest]
#[case::nontoken_creatures_you_control(
    "Nontoken creatures you control have riot.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Riot"}]}}"#
)]
#[case::creatures_you_control(
    "Creatures you control have riot.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Riot"}]}}"#
)]
fn riot_given_to_a_group_reads_as_a_static_ability(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::a_counter_on_it(
    "{3}{R}: Draw a card. Activate only if ~ has a +1/+1 counter on it.",
    ClauseKind::Activated
)]
#[case::a_counter_on_this_creature(
    "{3}{R}: ~ deals 2 damage to any target. Activate only if this creature has a +1/+1 counter on it.",
    ClauseKind::Activated
)]
fn the_wording_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[test]
fn an_unknown_word_after_have_is_left_to_the_table() {
    let text = "Nontoken creatures you control have rioting.";
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}
