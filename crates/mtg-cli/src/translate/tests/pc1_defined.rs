//! Goal: a `PutCounter` or `RemoveCounter` whose `Defined$` names the card or a player beside a target puts on what
//! `Defined$` names, and the target is only chosen (for the amount to read); `Defined$ Targeted` and no `Defined$` put
//! on the target.
use rstest::rstest;
use serde_json::{Value, json};

use super::abilities;

/// The first ability's body, under whichever kind it is.
fn body(script: &str) -> Value {
    let found = abilities(script);
    let first = found.first().cloned().unwrap_or_default();
    first
        .as_object()
        .and_then(|map| map.values().next().cloned())
        .unwrap_or_default()
}

fn walker(line: &str) -> String {
    format!(
        "Name:T\nTypes:Legendary Planeswalker Gideon\nLoyalty:4\nA:AB$ {line} | Planeswalker$ True\nSVar:X:TargetedPlayer$CreaturesInPlay\n"
    )
}

fn sorcery(line: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::the_card_beside_an_opponent(
    walker("PutCounter | Cost$ AddCounter<1/LOYALTY> | Defined$ Self | CounterType$ LOYALTY | CounterNum$ X | ValidTgts$ Opponent"),
    json!("This"),
    json!([{"what": "Opponent"}])
)]
#[case::you_beside_a_card_in_a_graveyard(
    format!(
        "{}SVar:DBReturn:DB$ ChangeZone | Defined$ Targeted | Origin$ Graveyard | Destination$ Hand\n",
        sorcery("PutCounter | Defined$ You | CounterType$ ENERGY | CounterNum$ 2 | ValidTgts$ Creature.YouOwn | TgtZone$ Graveyard | SubAbility$ DBReturn")
    ),
    json!("You"),
    json!([{"what": {"Object": [{"Type": "Creature"}, {"OwnedBy": "You"}, {"In": "Graveyard"}]}}])
)]
#[case::the_target_named_again(
    sorcery("PutCounter | Defined$ Targeted | CounterType$ P1P1 | CounterNum$ 1 | ValidTgts$ Creature"),
    json!({"Target": 0}),
    json!([{"what": {"Object": [{"Type": "Creature"}]}}])
)]
#[case::the_target_alone(
    sorcery("PutCounter | CounterType$ P1P1 | CounterNum$ 1 | ValidTgts$ Creature"),
    json!({"Target": 0}),
    json!([{"what": {"Object": [{"Type": "Creature"}]}}])
)]
fn a_defined_word_beside_a_target_names_what_gets_the_counters(
    #[case] script: String,
    #[case] on: Value,
    #[case] targets: Value,
) {
    let body = body(&script);
    assert_eq!(body["targets"], targets, "{body}");
    assert_eq!(body["effects"][0]["PutCounters"]["on"], on, "{body}");
}
