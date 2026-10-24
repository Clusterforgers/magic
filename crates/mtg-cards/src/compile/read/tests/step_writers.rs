//! Goal: the writers of the proposal checker give the oracle wording of a step on a comparison of the creature of an event with the card and the step taken otherwise ("put a +1/+1 counter on that creature if its power is less than ~'s power. Otherwise, ..."), of a choice of two modes of one target that differ in a phrase ("target creature gets +2/-2 or -2/+2 until end of turn") and of a bigger step on a second count ("If seven or more creatures died this turn, instead ..."), and a wrong relation, characteristic, step, count, target or duration never reads back.
use super::*;

fn died(count: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Tallied":{{"who":"EachPlayer","what":"CreaturesDied"}}}},"op":"Ge","right":{count}}}}}"#
    )
}

fn chant(first: u32, second: u32, more: &str, usual: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"End","whose":"You"}}}},"if":{},"effects":[{{"If":{{"condition":{},"then":{more},"otherwise":{usual}}}}}]}}}}"#,
        died(first),
        died(second)
    )
}

const SEVEN_CARDS: &str = r#"[{"Draw":{"count":7}},{"LoseLife":{"amount":7}}]"#;
const ONE_CARD: &str = r#"[{"Draw":{}},{"LoseLife":{"amount":1}}]"#;
const CHANT: &str = "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 1 life. If seven or more creatures died this turn, instead you draw seven cards and you lose 7 life.";

#[test]
fn a_bigger_step_on_a_second_count_is_written_as_the_oracle_words_it() {
    assert!(written(&chant(1, 7, SEVEN_CARDS, ONE_CARD), CHANT));
}

#[rstest]
#[case::another_second_count(
    "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 1 life. If six or more creatures died this turn, instead you draw seven cards and you lose 7 life."
)]
#[case::another_bigger_step(
    "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 1 life. If seven or more creatures died this turn, instead you draw seven cards and you lose 6 life."
)]
#[case::another_usual_step(
    "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 2 life. If seven or more creatures died this turn, instead you draw seven cards and you lose 7 life."
)]
#[case::another_first_count(
    "At the beginning of your end step, if two or more creatures died this turn, you draw a card and you lose 1 life. If seven or more creatures died this turn, instead you draw seven cards and you lose 7 life."
)]
#[case::without_instead(
    "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 1 life. If seven or more creatures died this turn, you draw seven cards and you lose 7 life."
)]
#[case::the_usual_step_alone(
    "At the beginning of your end step, if a creature died this turn, you draw a card and you lose 1 life."
)]
fn a_bigger_step_on_a_second_count_does_not_read_as_another_pair_of_steps(#[case] clause: &str) {
    assert!(
        !written(&chant(1, 7, SEVEN_CARDS, ONE_CARD), clause),
        "{clause}"
    );
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn counter_by(op: &str, stat: &str, chosen: &str, otherwise: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},"Other",{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"If":{{"condition":{{"Compare":{{"left":{{"Stat":{{"of":"Triggering","stat":"{stat}"}}}},"op":"{op}","right":{{"Stat":{{"of":"This","stat":"{stat}"}}}}}}}},"then":[{{"PutCounters":{{"on":{chosen},"counter":"+1/+1"}}}}],"otherwise":[{{"PutCounters":{{"on":{otherwise},"counter":"+1/+1"}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::less_power(
    counter_by("Lt", "Power", r#""Triggering""#, r#""This""#),
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its power is less than ~'s power. Otherwise, put a +1/+1 counter on ~."
)]
#[case::greater_toughness(
    counter_by("Gt", "Toughness", r#""Triggering""#, r#""This""#),
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its toughness is greater than ~'s toughness. Otherwise, put a +1/+1 counter on ~."
)]
#[case::at_least(
    counter_by("Ge", "Power", r#""Triggering""#, r#""This""#),
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its power is greater than or equal to ~'s power. Otherwise, put a +1/+1 counter on ~."
)]
fn a_step_on_a_comparison_and_the_other_one_are_written_as_the_oracle_words_them(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_relation(
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its power is greater than ~'s power. Otherwise, put a +1/+1 counter on ~."
)]
#[case::another_characteristic(
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its toughness is less than ~'s toughness. Otherwise, put a +1/+1 counter on ~."
)]
#[case::mixed_characteristics(
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its power is less than ~'s toughness. Otherwise, put a +1/+1 counter on ~."
)]
#[case::the_steps_swapped(
    "Whenever another creature you control enters, put a +1/+1 counter on ~ if its power is less than ~'s power. Otherwise, put a +1/+1 counter on that creature."
)]
#[case::no_otherwise(
    "Whenever another creature you control enters, put a +1/+1 counter on that creature if its power is less than ~'s power."
)]
#[case::another_counter(
    "Whenever another creature you control enters, put a -1/-1 counter on that creature if its power is less than ~'s power. Otherwise, put a +1/+1 counter on ~."
)]
fn a_step_on_a_comparison_does_not_read_as_another_comparison_or_step(#[case] clause: &str) {
    let json = counter_by("Lt", "Power", r#""Triggering""#, r#""This""#);
    assert!(!written(&json, clause), "{clause}");
}

fn either_mode(second: &str, second_target: &str, duration: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"TurnedFaceUp":{{"what":["This"]}}}},"effects":[{{"Modal":{{"modes":[{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"ModifyPT":{{"power":2,"toughness":-2}}}}],"duration":"UntilEndOfTurn"}}}}]}},{{"targets":[{{"what":{{"Object":[{second_target}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"ModifyPT":{second}}}],"duration":"{duration}"}}}}]}}]}}}}]}}}}"#
    )
}

const MINUS_PLUS: &str = r#"{"power":-2,"toughness":2}"#;
const CREATURE: &str = r#"{"Type":"Creature"}"#;
const EITHER: &str =
    "When this creature is turned face up, target creature gets +2/-2 or -2/+2 until end of turn.";

#[test]
fn two_modes_of_one_target_that_differ_in_a_phrase_are_one_sentence() {
    assert!(written(
        &either_mode(MINUS_PLUS, CREATURE, "UntilEndOfTurn"),
        EITHER
    ));
}

#[rstest]
#[case::the_order_swapped(
    "When this creature is turned face up, target creature gets -2/+2 or +2/-2 until end of turn."
)]
#[case::both_at_once(
    "When this creature is turned face up, target creature gets +2/-2 and -2/+2 until end of turn."
)]
#[case::another_duration(
    "When this creature is turned face up, target creature gets +2/-2 or -2/+2 until end of your next turn."
)]
#[case::another_bonus(
    "When this creature is turned face up, target creature gets +2/-2 or -3/+3 until end of turn."
)]
#[case::another_target(
    "When this creature is turned face up, target creature you control gets +2/-2 or -2/+2 until end of turn."
)]
fn two_modes_of_one_target_do_not_read_as_another_choice(#[case] clause: &str) {
    let json = either_mode(MINUS_PLUS, CREATURE, "UntilEndOfTurn");
    assert!(!written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_target_kind(r#"{"Type":"Artifact"}"#, "UntilEndOfTurn")]
#[case::another_duration(CREATURE, "UntilEndOfCombat")]
fn two_modes_with_another_target_or_duration_are_not_one_sentence(
    #[case] target: &str,
    #[case] duration: &str,
) {
    assert!(!written(&either_mode(MINUS_PLUS, target, duration), EITHER));
}
