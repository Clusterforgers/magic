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
#[case::other_cast_count(
    "K:etbCounter:P1P1:1:CheckSVar$ C:x\nSVar:C:Count$wasCastFromYourHandByYou.2.0\n",
    "amount:Count$wasCastFromYourHandByYou"
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
