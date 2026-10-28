//! Goal: the counters a player gets by word ("you get two rad counters", "each player gets a rad counter", "target player gets four rad counters", "they get that many rad counters") read into `PutCounters` on the player and write back the same way, and other kinds, counts and players stay with the table.
use super::*;

#[rstest]
#[case::you_in_a_trigger(
    "When this creature enters, you get two rad counters.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"rad"},"count":2}}]}}"#
)]
#[case::each_player(
    "Whenever this creature attacks, each player gets two rad counters.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"EachPlayer","counter":{"Other":"rad"},"count":2}}]}}"#
)]
#[case::target_player(
    "When this creature enters, target player gets two rad counters.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"rad"},"count":2}}]}}"#
)]
#[case::each_opponent_poison(
    "Each opponent gets a poison counter.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"Opponents","counter":"poison"}}]}}"#
)]
#[case::you_experience(
    "You get an experience counter.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"You","counter":{"Other":"experience"}}}]}}"#
)]
fn a_player_gets_counters_by_word(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::a_kind_a_player_does_not_hold("You get two +1/+1 counters.")]
#[case::a_named_object_kind("Each player gets a charge counter.")]
#[case::no_count("You get rad counters.")]
#[case::the_wrong_number_word("You get two rad counter.")]
fn other_counters_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
