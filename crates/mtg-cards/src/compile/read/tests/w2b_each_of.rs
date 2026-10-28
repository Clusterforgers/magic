//! Goal: "Put a +1/+1 counter on each of up to two target creatures." and its forms with several target objects read as one `PutCounters` on the targets and write back as the card's text; one target after "each of", a player or a missing "each of" stays with the table or reads as before.
use super::*;

#[rstest]
#[case::up_to_two(
    "Put a +1/+1 counter on each of up to two target creatures.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#
)]
#[case::exactly_two(
    "Put two +1/+1 counters on each of two target creatures.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":2,"max":2}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}]}}"#
)]
fn counters_go_on_each_of_several_targets(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::one_target("Put a +1/+1 counter on each of target creature.")]
#[case::players("Put a +1/+1 counter on each of up to two target players.")]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
