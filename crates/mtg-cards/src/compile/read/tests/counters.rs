//! Goal: "put a +1/+1 counter on ~" and its kin read into `PutCounters` on the card or on one target with the counts and the two sizes of counter, and any other counter, count or place stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[test]
fn tireless_tracker_reads_in_the_shape_of_its_file() {
    reads_as(
        "Whenever you sacrifice a Clue, put a +1/+1 counter on ~.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"Sacrificed":{"what":[{"Subtype":"Clue"}],"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
    );
}

#[rstest]
#[case::on_a_target(
    "Put a +1/+1 counter on target creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#
)]
#[case::two(
    "Put two +1/+1 counters on target creature you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}]}}"#
)]
#[case::minus(
    "Put a -1/-1 counter on target creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"-1/-1"}}]}}"#
)]
fn counters_read_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Put a +1/+1 counter on target creature.")]
#[case("Put a +1/+1 counter on target creature an opponent controls.")]
#[case("Put three +1/+1 counters on target creature.")]
#[case("Put two -1/-1 counters on target creature.")]
#[case("Put a +1/+1 counter on target artifact.")]
#[case("Put a +1/+1 counter on ~.")]
#[case("Put two +1/+1 counters on ~.")]
#[case("Put a -1/-1 counter on ~.")]
#[case("You may put a +1/+1 counter on target creature.")]
#[case("Draw a card. Put a +1/+1 counter on ~.")]
#[case("Put a +1/+1 counter on target creature. You gain 1 life.")]
fn more_oracle_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, put a +1/+1 counter on target creature.")]
#[case("When ~ enters, put two +1/+1 counters on ~.")]
#[case("Whenever ~ attacks, put a +1/+1 counter on ~.")]
#[case("Whenever a creature you control dies, put a +1/+1 counter on ~.")]
#[case("Whenever you cast a creature spell, you may put a +1/+1 counter on ~.")]
#[case("Whenever you sacrifice a Clue, put a +1/+1 counter on ~.")]
#[case("At the beginning of your upkeep, put a +1/+1 counter on target creature you control.")]
#[case("At the beginning of your end step, put a -1/-1 counter on target creature.")]
#[case("Landfall \u{2014} Whenever a land you control enters, put a +1/+1 counter on ~.")]
fn more_oracle_wordings_in_a_trigger_read_and_write_back(#[case] text: &str) {
    let rest = text.split_once(" \u{2014} ").map_or(text, |(_, rest)| rest);
    assert_eq!(
        trigger(text),
        Some(golden(rest, ClauseKind::Triggered, false)),
        "{text}"
    );
}

#[rstest]
#[case::each("Put a +1/+1 counter on each creature you control.")]
#[case::up_to("Put a +1/+1 counter on up to one target creature.")]
#[case::another_counter("Put a charge counter on ~.")]
#[case::another_size("Put a +2/+2 counter on ~.")]
#[case::a_mixed_size("Put a +1/-1 counter on ~.")]
#[case::for_each("Put a +1/+1 counter on ~ for each creature you control.")]
#[case::no_count("Put +1/+1 counters on ~.")]
#[case::a_plural_for_one("Put a +1/+1 counters on ~.")]
#[case::a_singular_for_many("Put two +1/+1 counter on ~.")]
#[case::digits("Put 2 +1/+1 counters on ~.")]
#[case::a_pronoun("Put a +1/+1 counter on it.")]
#[case::a_player("Put a +1/+1 counter on target player.")]
#[case::a_variable("Put X +1/+1 counters on ~.")]
#[case::a_second_target(
    "Put a +1/+1 counter on target creature and a -1/-1 counter on target land."
)]
#[case::remove("Remove a +1/+1 counter from ~.")]
#[case::move_counters("Move a +1/+1 counter from target creature onto ~.")]
#[case::double("Double the number of +1/+1 counters on ~.")]
#[case::no_place("Put a +1/+1 counter.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
