//! Goal: the keyword and cost words p8 maps mean what the card prints, and a shape the node cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn creature(lines: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\n{lines}")
}

#[rstest]
#[case::revolt(
    "K:etbCounter:P1P1:2:Revolt$ True:Disappear\n",
    json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "PermanentsLeftUnder"}}, "op": "Ge", "right": 1}}),
    json!("+1/+1"),
    2
)]
#[case::raid(
    "K:etbCounter:P1P1:1:CheckSVar$ RaidTest:Raid\nSVar:RaidTest:Count$AttackersDeclared\n",
    json!({"Compare": {"left": {"Tallied": {"who": "You", "what": "Attackers"}}, "op": "Ge", "right": 1}}),
    json!("+1/+1"),
    1
)]
#[case::adamant_white(
    "K:etbCounter:P1P1:1:Adamant$ White:x\n",
    json!({"Compare": {"left": {"ManaSpent": {"of": "White"}}, "op": "Ge", "right": 3}}),
    json!("+1/+1"),
    1
)]
#[case::cast_by_you(
    "K:etbCounter:REVIVAL:8:ValidCard$ Card.Self+wasCastByYou:x\n",
    json!({"Matches": {"subject": "This", "selector": ["This", {"WasCast": {"by": "You"}}]}}),
    json!({"Other": "revival"}),
    8
)]
#[case::cast_from_hand(
    "K:etbCounter:DIVINITY:1:CheckSVar$ FromHand:x\nSVar:FromHand:Count$wasCastFromYourHandByYou.1.0\n",
    json!({"Matches": {"subject": "This", "selector": ["This", {"WasCast": {"by": "You", "from": "Hand"}}, {"OwnedBy": "You"}]}}),
    json!({"Other": "divinity"}),
    1
)]
fn an_etb_counter_condition_is_the_replacements_condition(
    #[case] lines: &str,
    #[case] condition: Value,
    #[case] kind: Value,
    #[case] count: u32,
) {
    let found = abilities(&creature(lines));
    let expected = normal(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": kind, "count": count}},
        "condition": condition}}));
    assert_eq!(found, vec![expected]);
}

#[rstest]
#[case::indestructible("Indestructible", json!({"Other": "indestructible"}))]
#[case::flying("Flying", json!({"Other": "flying"}))]
#[case::first_strike("First Strike", json!({"Other": "first strike"}))]
fn a_counter_named_for_a_keyword_is_the_engines_keyword_counter(
    #[case] word: &str,
    #[case] kind: Value,
) {
    let found = abilities(&creature(&format!("K:etbCounter:{word}:1\n")));
    let expected = normal(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": kind, "count": 1}}}}));
    assert_eq!(found, vec![expected]);
}

#[rstest]
#[case::unknown_field(
    "K:etbCounter:P1P1:1:Hellbent$ True:x\n",
    "keyword:etbCounter-condition"
)]
#[case::self_property(
    "K:etbCounter:P1P1:1:ValidCard$ Card.Self+untapped:x\n",
    "keyword:etbCounter-condition"
)]
fn an_etb_condition_the_node_cannot_say_stays_a_gap(#[case] lines: &str, #[case] gap: &str) {
    let script = creature(lines);
    assert_eq!(gaps(&script), vec![gap.to_owned()]);
    assert_eq!(abilities(&script), Vec::<Value>::new());
}

const CLASS: &str = "Name:T\nTypes:Enchantment Class\nK:Class:2:3 W:AddTrigger$ Trig\nSVar:Trig:Mode$ LifeGained | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Put | Secondary$ True | TriggerDescription$ Whenever you gain life, put a +1/+1 counter on target creature you control.\nSVar:Put:DB$ PutCounter | ValidTgts$ Creature.YouCtrl | CounterType$ P1P1 | CounterNum$ 1\n";

#[test]
fn a_class_level_is_a_sorcery_speed_step_and_an_ability_that_is_off_below_it() {
    let level_up = json!({"Activated": {
        "cost": "{3}{W}", "timing": "Sorcery",
        "condition": {"Level": {"of": "This", "op": "Eq", "level": 1}},
        "effects": [{"BecomeLevel": {"level": 2}}]}});
    let added = json!({"Triggered": {
        "when": {"LifeGained": {"who": "You"}},
        "if": {"Level": {"of": "This", "op": "Ge", "level": 2}},
        "targets": [{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}],
        "effects": [{"PutCounters": {"on": {"Target": 0}, "counter": "+1/+1"}}]}});
    assert_eq!(abilities(CLASS), vec![normal(level_up), normal(added)]);
}

#[test]
fn a_class_static_ability_holds_only_at_its_level() {
    let script = "Name:T\nTypes:Enchantment Class\nK:Class:3:2 G:AddStaticAbility$ Pump\nSVar:Pump:Mode$ Continuous | Affected$ Creature.YouCtrl | AddPower$ 1 | Secondary$ True | Description$ Creatures you control get +1/+0.\n";
    let found = abilities(script);
    let held = json!({"Level": {"of": "This", "op": "Ge", "level": 3}});
    assert_eq!(found.len(), 2);
    assert_eq!(found[1]["Static"]["condition"], held);
}

#[rstest]
#[case::first_level("K:Class:1:2 G:AddTrigger$ Trig\n", "keyword:Class-level")]
#[case::unknown_add("K:Class:2:2 G:AddSVar$ Trig\nSVar:Trig:X\n", "keyword:Class-AddSVar")]
#[case::no_price("K:Class:2\n", "keyword:Class-shape")]
fn a_class_line_the_node_cannot_say_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = format!("Name:T\nTypes:Enchantment Class\n{line}");
    assert!(
        gaps(&script).contains(&gap.to_owned()),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::black("Hexproof:Black", json!({"HexproofFrom": [{"Color": "Black"}]}))]
#[case::artifact("Hexproof:Artifact", json!({"HexproofFrom": [{"Type": "Artifact"}]}))]
fn hexproof_from_a_quality_keeps_its_selector(#[case] word: &str, #[case] node: Value) {
    let found = abilities(&creature(&format!("K:{word}\n")));
    assert_eq!(found, vec![normal(json!({ "Keyword": node }))]);
}

#[test]
fn hexproof_from_triggered_abilities_stays_a_gap() {
    let script = creature("K:Hexproof:Triggered:triggered abilities\n");
    assert_eq!(abilities(&script), Vec::<Value>::new());
    assert_eq!(gaps(&script).len(), 1);
}

const KICKED: &str = "R:Event$ Moved | ValidCard$ Card.Self+kicked | Destination$ Battlefield | ReplaceWith$ Pump | ReplacementResult$ Updated\n";

#[test]
fn a_kicked_replacement_that_puts_counters_is_conditional_on_the_kick() {
    let body = "SVar:Pump:DB$ PutCounter | ETB$ True | Defined$ Self | CounterType$ P1P1 | CounterNum$ 3\n";
    let expected = normal(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"WithCounters": {"counter": "+1/+1", "count": 3}},
        "condition": "Kicked"}}));
    assert_eq!(
        abilities(&creature(&format!("{KICKED}{body}"))),
        vec![expected]
    );
}

#[test]
fn a_kicked_replacement_with_a_second_body_stays_a_gap() {
    let body = "SVar:Pump:DB$ PutCounter | ETB$ True | Defined$ Self | CounterType$ P1P1 | CounterNum$ 2 | SubAbility$ Haste\nSVar:Haste:DB$ Animate | Defined$ Self | Keywords$ Haste | Duration$ Permanent\n";
    let found = gaps(&creature(&format!("{KICKED}{body}")));
    assert_eq!(found, vec!["key:Replacement.SubAbility".to_owned()]);
}
