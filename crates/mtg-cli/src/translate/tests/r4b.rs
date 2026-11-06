//! Goal: the Effect forms of round 4 are table rows: a remembered `CantRegenerate` static is the rule `CantBeRegenerated`
//! for the objects the effect remembers, for the duration of the effect.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::target(
    "Effect | ValidTgts$ Creature | RememberObjects$ Targeted | ForgetOnMoved$ Battlefield | StaticAbilities$ NoRegen\nSVar:NoRegen:Mode$ CantRegenerate | ValidCard$ Card.IsRemembered",
    json!([{"Rule": {"rule": {"CantBeRegenerated": {"Target": 0}}, "duration": "UntilEndOfTurn"}}])
)]
fn a_remembered_cant_regenerate_static_is_a_rule(#[case] script: &str, #[case] expected: Value) {
    assert_eq!(effects(&spell(script)), expected);
}

#[rstest]
#[case::every_kind(
    "RemoveCounterAll | ValidCards$ Permanent | AllCounterTypes$ True",
    json!({"RemoveAllCounters": {"from": {"Each": []}}})
)]
#[case::all_of_a_kind(
    "RemoveCounterAll | ValidCards$ Creature | CounterType$ GOLD | AllCounters$ True",
    json!({"RemoveAllCounters": {"from": {"Each": [{"Type": "Creature"}]}, "counter": {"Other": "gold"}}})
)]
#[case::no_count_is_all_of_the_kind(
    "RemoveCounterAll | ValidCards$ Permanent | CounterType$ RUST",
    json!({"RemoveAllCounters": {"from": {"Each": []}, "counter": {"Other": "rust"}}})
)]
#[case::a_number_from_each(
    "RemoveCounterAll | ValidCards$ Creature.YouCtrl | CounterType$ M1M1 | CounterNum$ 1",
    json!({"RemoveCounters": {"from": {"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "counter": "-1/-1"}})
)]
fn remove_counter_all_reads_its_count_words(#[case] line: &str, #[case] node: Value) {
    assert_eq!(effects(&spell(line)), json!([node]));
}

#[rstest]
#[case::cards_in_exile(
    "RemoveCounterAll | ValidCards$ Card.suspended | CounterType$ TIME | CounterNum$ 2 | ValidZone$ Exile",
    "key:RemoveCounterAll.ValidZone"
)]
fn remove_counter_all_leaves_what_it_cannot_read_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::two_alternatives(
    "DigMultiple | DigNum$ 4 | ChangeValid$ Creature,Land | Optional$ True",
    2
)]
#[case::four_alternatives(
    "DigMultiple | DigNum$ 4 | ChangeValid$ Creature,Land,Artifact,Enchantment | Optional$ True | DestinationZone2$ Library",
    4
)]
fn dig_multiple_takes_one_card_for_each_alternative(
    #[case] line: &str,
    #[case] alternatives: usize,
) {
    let found = effects(&spell(line));
    let steps = found.as_array().unwrap();
    let choices: Vec<&Value> = steps.iter().filter_map(|step| step.get("Choose")).collect();
    assert_eq!(choices.len(), alternatives);
    for (index, choice) in choices.iter().enumerate() {
        assert_eq!(choice["count"], json!({"min": 0, "max": 1}));
        // Each later alternative leaves out the cards the earlier ones took.
        let pool = choice["from"]["Each"].as_array().unwrap();
        let excluded = pool.iter().filter(|pred| pred.get("Not").is_some()).count();
        assert_eq!(excluded, index);
    }
}

#[rstest]
#[case::the_source_itself(
    "AddOrRemoveCounter | Defined$ Self | CounterType$ PLAGUE | CounterNum$ 1",
    json!({"OneOf": {"options": [
        [{"PutCounters": {"on": "This", "counter": {"Other": "plague"}}}],
        [{"RemoveCounters": {"from": "This", "counter": {"Other": "plague"}}}]]}})
)]
#[case::optional(
    "AddOrRemoveCounter | Defined$ Self | CounterType$ DOOM | Optional$ True",
    json!({"OneOf": {"options": [
        [{"PutCounters": {"on": "This", "counter": {"Other": "doom"}}}],
        [{"RemoveCounters": {"from": "This", "counter": {"Other": "doom"}}}],
        []]}})
)]
fn add_or_remove_counter_is_a_choice_of_putting_or_taking(#[case] line: &str, #[case] node: Value) {
    assert_eq!(effects(&spell(line)), json!([node]));
}

#[rstest]
#[case::each_kind(
    "AddOrRemoveCounter | ValidTgts$ Permanent | EachExistingCounter$ True | CounterNum$ 1",
    "key:AddOrRemoveCounter.EachExistingCounter"
)]
fn add_or_remove_counter_leaves_what_it_cannot_read_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(gaps(&spell(line)).contains(&gap.to_owned()));
}

#[rstest]
#[case::each_player(
    "Radiation | Defined$ Player | Num$ 4",
    json!({"PutCounters": {"on": "EachPlayer", "counter": {"Other": "rad"}, "count": 4}})
)]
#[case::you("Radiation | Defined$ You | Num$ 2", json!({"PutCounters": {"on": "You", "counter": {"Other": "rad"}, "count": 2}}))]
#[case::opponents(
    "Radiation | Defined$ Opponent | Num$ 1",
    json!({"PutCounters": {"on": "Opponents", "counter": {"Other": "rad"}}})
)]
fn radiation_puts_rad_counters_on_players(#[case] line: &str, #[case] node: Value) {
    assert_eq!(effects(&spell(line)), json!([node]));
}

#[rstest]
#[case::one_turn("SkipTurn | Defined$ You | NumTurns$ 1", 1)]
#[case::two_turns("SkipTurn | Defined$ You | NumTurns$ 2", 2)]
fn skip_turn_is_a_rule_for_each_turn_skipped(#[case] line: &str, #[case] turns: usize) {
    let rule = json!({"Rule": {
        "rule": {"Cant": {"who": "You", "action": "SkipTurn"}}, "duration": "UntilTurnSkipped"}});
    assert_eq!(effects(&spell(line)), json!(vec![rule; turns]));
}

#[rstest]
#[case::not_optional(
    "DigMultiple | DigNum$ 4 | ChangeValid$ Creature,Land",
    "key:DigMultiple.Optional-absent"
)]
#[case::other_zone(
    "DigMultiple | DigNum$ 4 | ChangeValid$ Creature,Land | Optional$ True | SourceZone$ Graveyard",
    "key:DigMultiple.SourceZone"
)]
fn dig_multiple_leaves_what_it_cannot_read_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}
