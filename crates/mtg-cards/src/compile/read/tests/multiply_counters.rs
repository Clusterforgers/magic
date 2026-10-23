//! Goal: doubling and tripling the counters of one kind or of each kind on the card itself, a target, a group of creatures or the player reads into `MultiplyCounters`, under a trigger, a cost or a "may", and writes back as the same text; a count, a place or a kind this does not cover stays with the table.
use super::*;

fn effect(on: &str, counter: &str, factor: &str) -> String {
    format!(r#"{{"MultiplyCounters":{{"on":{on}{counter}{factor}}}}}"#)
}

fn spell(targets: &str, effect: &str) -> String {
    format!(r#"{{"Spell":{{{targets}"effects":[{effect}]}}}}"#)
}

const CREATURE_TARGET: &str = r#""targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"#;

#[rstest]
#[case::the_card_itself(
    "Double the number of +1/+1 counters on ~.",
    spell("", &effect(r#""This""#, r#","counter":"+1/+1""#, ""))
)]
#[case::each_kind_on_a_target(
    "Double the number of each kind of counter on target creature.",
    spell(CREATURE_TARGET, &effect(r#"{"Target":0}"#, "", ""))
)]
#[case::tripling(
    "Triple the number of +1/+1 counters on target creature.",
    spell(CREATURE_TARGET, &effect(r#"{"Target":0}"#, r#","counter":"+1/+1""#, r#","factor":3"#))
)]
#[case::each_creature_you_control(
    "Double the number of +1/+1 counters on each creature you control.",
    spell("", &effect(r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#, r#","counter":"+1/+1""#, ""))
)]
#[case::the_counters_you_have(
    "Double the number of each kind of counter you have.",
    spell("", &effect(r#""You""#, "", ""))
)]
fn multiplying_counters_reads_and_writes_back(#[case] text: &str, #[case] json: String) {
    reads_as(text, ClauseKind::Spell, false, &json);
}

#[rstest]
#[case::attacking(
    "Whenever ~ attacks, double the number of +1/+1 counters on it.",
    ClauseKind::Triggered
)]
#[case::at_the_upkeep(
    "At the beginning of your upkeep, double the number of +1/+1 counters on ~.",
    ClauseKind::Triggered
)]
#[case::entering(
    "When ~ enters, double the number of +1/+1 counters on target creature you control.",
    ClauseKind::Triggered
)]
#[case::an_activated_ability(
    "{G}{U}, {T}: Double the number of each kind of counter on target artifact, creature, or land.",
    ClauseKind::Activated
)]
fn the_clause_under_a_trigger_or_a_cost_reads_and_writes_back(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    golden(text, kind, false);
}

#[rstest]
#[case::another_factor("Quadruple the number of +1/+1 counters on ~.")]
#[case::half("Halve the number of +1/+1 counters on ~.")]
#[case::more_than_one_target(
    "Double the number of each kind of counter on any number of target permanents."
)]
#[case::the_counters_of_a_player_kind("Double the number of poison counters you have.")]
#[case::all_but_the_counters(
    "Double the number of +1/+1 counters on ~ if its power is 16 or less."
)]
fn a_wording_of_another_factor_place_or_condition_does_not_read(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}
