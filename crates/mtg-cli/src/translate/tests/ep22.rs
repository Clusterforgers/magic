//! Goal: the pieces of ep22 translate as the card prints them: a leave trigger printed "if it had counters on it" reads its counters as the intervening condition on the object of the event and keeps the rest of the filter, while one printed "with a counter on it" keeps the filter; other filters, a move into the battlefield and a list of alternatives stay as they were.
use rstest::rstest;
use serde_json::{Value, json};

use super::abilities;

fn trigger(line: &str, printed: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nT:{line} | TriggerZones$ Battlefield | Execute$ Draw | TriggerDescription$ {printed}\nSVar:Draw:DB$ Draw\n"
    )
}

fn triggered(script: &str) -> Value {
    abilities(script)
        .into_iter()
        .next()
        .and_then(|found| found.get("Triggered").cloned())
        .unwrap_or_default()
}

const LEAVES: &str = "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Creature.YouCtrl+HasCounters";
const IF_HAD: &str = "Whenever a creature you control leaves the battlefield, if it had counters on it, draw a card.";
const WITH: &str =
    "Whenever a creature you control with a counter on it leaves the battlefield, draw a card.";

#[rstest]
#[case::printed_if(LEAVES, IF_HAD, false, true)]
#[case::printed_with(LEAVES, WITH, true, false)]
#[case::into_the_battlefield(
    "Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Creature.YouCtrl+HasCounters",
    IF_HAD,
    true,
    false
)]
#[case::alternatives(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Creature.YouCtrl,Artifact.YouCtrl+HasCounters",
    IF_HAD,
    true,
    false
)]
fn ep22_had_counters_is_the_intervening_condition(
    #[case] line: &str,
    #[case] printed: &str,
    #[case] in_filter: bool,
    #[case] as_if: bool,
) {
    let found = triggered(&trigger(line, printed));
    let when = found.get("when").map(Value::to_string).unwrap_or_default();
    assert_eq!(when.contains("HasAnyCounter"), in_filter, "{found}");
    let had = json!({"Matches": {"subject": "Triggering", "selector": ["HasAnyCounter"]}});
    assert_eq!(found.get("if") == Some(&had), as_if, "{found}");
}
