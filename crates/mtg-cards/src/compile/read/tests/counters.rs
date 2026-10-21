//! Goal: putting, removing and moving counters read into `PutCounters`, `RemoveCounters` and `MoveCounters` on the card, on one target or (for putting) on each object of a phrase, with the counts, the two sizes of counter and the named kinds, and any other count, place or order stays with the table.
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
#[case::up_to("Put a +1/+1 counter on up to one target creature.")]
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
#[case::double("Double the number of +1/+1 counters on ~.")]
#[case::no_place("Put a +1/+1 counter.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::a_named_kind(
    "Put a charge counter on ~.",
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"This","counter":"charge"}}]}}"#
)]
#[case::another_kind(
    "Put two time counters on target artifact.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"time"},"count":2}}]}}"#
)]
#[case::on_each(
    "Put a +1/+1 counter on each creature you control.",
    r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}]}}"#
)]
#[case::remove_from_the_card(
    "Remove a +1/+1 counter from ~.",
    r#"{"Spell":{"effects":[{"RemoveCounters":{"from":"This","counter":"+1/+1"}}]}}"#
)]
#[case::remove_from_a_target(
    "Remove two charge counters from target permanent.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":"charge","count":2}}]}}"#
)]
#[case::move_onto_the_card(
    "Move a +1/+1 counter from target creature onto ~.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"MoveCounters":{"from":{"Target":0},"to":"This","counter":"+1/+1"}}]}}"#
)]
#[case::move_off_the_card(
    "Move three -1/-1 counters from ~ onto target creature an opponent controls.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"MoveCounters":{"from":"This","to":{"Target":0},"counter":"-1/-1","count":3}}]}}"#
)]
fn removing_and_moving_counters_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

/// Every kind of counter and count with every place, in a spell, a trigger and an activated ability.
#[test]
fn every_counter_kind_count_and_place_writes_back() {
    let kinds = [
        "+1/+1", "-1/-1", "charge", "poison", "loyalty", "time", "storage", "spore",
    ];
    let counts = ["a", "two", "three", "four", "five", "ten", "twenty"];
    let objects = [
        "~",
        "target creature",
        "target creature you control",
        "target nonland permanent",
        "target artifact or creature",
    ];
    let mut read = 0;
    for kind in kinds {
        for count in counts {
            let noun = if count == "a" { "counter" } else { "counters" };
            let counters = format!("{count} {kind} {noun}");
            let mut wordings = vec![
                format!("put {counters} on each creature you control"),
                format!("put {counters} on each land"),
            ];
            for object in objects {
                wordings.push(format!("put {counters} on {object}"));
                wordings.push(format!("remove {counters} from {object}"));
                for other in objects {
                    // One of the two is the card, and the other a target at most.
                    if (object == "~") != (other == "~") {
                        wordings.push(format!("move {counters} from {object} onto {other}"));
                    }
                }
            }
            for wording in wordings {
                let sentence = format!("{}.", capitalize(&wording));
                golden(&sentence, ClauseKind::Spell, false);
                golden(
                    &format!("When ~ enters, {wording}."),
                    ClauseKind::Triggered,
                    false,
                );
                golden(
                    &format!("{{2}}, {{T}}: {sentence}"),
                    ClauseKind::Activated,
                    false,
                );
                golden(
                    &format!("Whenever ~ attacks, you may {wording}."),
                    ClauseKind::Triggered,
                    false,
                );
                read += 4;
            }
        }
    }
    assert!(read > 4000, "only {read} wordings were read");
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[rstest]
#[case::all_of_them("Remove all +1/+1 counters from ~.")]
#[case::a_variable("Remove X +1/+1 counters from ~.")]
#[case::any_kind("Remove a counter from ~.")]
#[case::up_to("Remove up to two +1/+1 counters from ~.")]
#[case::one_for_a("Remove one +1/+1 counter from ~.")]
#[case::a_plural_for_one("Remove a +1/+1 counters from ~.")]
#[case::a_singular_for_many("Remove two +1/+1 counter from ~.")]
#[case::digits("Remove 2 +1/+1 counters from ~.")]
#[case::each("Remove a +1/+1 counter from each creature you control.")]
#[case::a_pronoun("Remove a +1/+1 counter from it.")]
#[case::no_place("Remove a +1/+1 counter.")]
#[case::move_between_targets(
    "Move a +1/+1 counter from target creature onto another target creature."
)]
#[case::move_onto_itself("Move a +1/+1 counter from ~ onto ~.")]
#[case::move_between_two_others("Move a +1/+1 counter from target creature onto target land.")]
#[case::move_all("Move all counters from target creature onto ~.")]
#[case::move_to_the_wrong_word("Move a +1/+1 counter from target creature to ~.")]
#[case::move_from_nowhere("Move a +1/+1 counter onto ~.")]
#[case::put_on_each_in_the_plural("Put a +1/+1 counter on each creatures you control.")]
#[case::a_kind_that_is_a_name("Put a Foo counter on ~.")]
#[case::a_kind_that_is_a_number("Put a 3 counter on ~.")]
#[case::no_kind("Put a counter on ~.")]
fn counters_the_templates_do_not_model_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
