//! Goal: "the number of times this creature has mutated" is an amount of its own; it reads after "where X is" and "equal to" in a trigger, an activated ability and a spell effect, becomes the `Mutations` count of the card, writes back as the same text, and a count of another creature or a different wording stays with the table.
use super::*;

fn kind_of(text: &str) -> ClauseKind {
    if text.starts_with('{') {
        ClauseKind::Activated
    } else if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    }
}

#[rstest]
#[case::life_and_gain(
    "Whenever this creature mutates, each opponent loses X life and you gain X life, where X is the number of times this creature has mutated."
)]
#[case::pump_the_others(
    "Whenever this creature mutates, other creatures you control get +X/+X until end of turn, where X is the number of times this creature has mutated."
)]
#[case::damage_when_tapped(
    "{T}: This creature deals X damage to any target, where X is the number of times this creature has mutated."
)]
#[case::the_name(
    "{T}: ~ deals X damage to any target, where X is the number of times ~ has mutated."
)]
#[case::counters_on_attack(
    "Whenever this creature attacks, put X +1/+1 counters on it, where X is the number of times this creature has mutated."
)]
#[case::equal_to_for_damage(
    "{T}: This creature deals damage equal to the number of times this creature has mutated to any target."
)]
#[case::equal_to_for_life(
    "Whenever this creature mutates, you gain life equal to the number of times this creature has mutated."
)]
fn the_number_of_times_a_creature_has_mutated_reads_and_writes_back(#[case] text: &str) {
    golden(text, kind_of(text), false);
}

#[rstest]
#[case::in_a_trigger(
    "Whenever this creature mutates, each opponent loses X life and you gain X life, where X is the number of times this creature has mutated."
)]
#[case::in_an_activation(
    "{T}: This creature deals X damage to any target, where X is the number of times this creature has mutated."
)]
fn the_amount_counts_the_cards_merged_into_the_card(#[case] text: &str) {
    let json = serde_json::to_string(&golden(text, kind_of(text), false)).expect("json");
    assert!(json.contains(r#""Mutations":"This""#), "{text}: {json}");
    assert!(!json.contains(r#""X""#), "{text}: {json}");
}

#[rstest]
#[case::another_creature(
    "{T}: This creature deals X damage to any target, where X is the number of times target creature has mutated."
)]
#[case::the_past_tense(
    "{T}: This creature deals X damage to any target, where X is the number of times this creature has been mutated."
)]
#[case::without_the_number(
    "{T}: This creature deals X damage to any target, where X is the times this creature has mutated."
)]
#[case::a_different_event(
    "{T}: This creature deals X damage to any target, where X is the number of times this creature has attacked."
)]
#[case::no_x_in_the_sentence(
    "{T}: This creature deals 2 damage to any target, where X is the number of times this creature has mutated."
)]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, kind_of(text), false), None, "{text}");
}
