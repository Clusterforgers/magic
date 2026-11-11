//! Goal: the Attraction lines of Forge (`OpenAttraction`, a roll with `ToVisitYourAttractions$`, the `Visit` keyword of an Attraction and the open trigger from the Attraction deck) map to the DSL nodes the engine runs, and each near miss stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:S\nTypes:Sorcery\nA:SP$ {line}\n")
}

fn attraction(line: &str) -> String {
    format!("Name:A\nTypes:Artifact Attraction\nK:Visit:Go\nSVar:Go:DB$ {line}\n")
}

#[rstest]
#[case::open_one("OpenAttraction", json!([{"OpenAttraction": {}}]))]
#[case::open_two("OpenAttraction | Amount$ 2", json!([{"OpenAttraction": {"count": 2}}]))]
#[case::roll("RollDice | Sides$ 6 | ToVisitYourAttractions$ True", json!([{"RollToVisit": {}}]))]
#[case::roll_without_sides("RollDice | ToVisitYourAttractions$ True", json!([{"RollToVisit": {}}]))]
fn attraction_verbs_map(#[case] line: &str, #[case] effects: Value) {
    let expected = normal(json!({"Spell": {"effects": effects}}));
    assert_eq!(abilities(&spell(line)), vec![expected]);
}

#[rstest]
#[case::another_player("OpenAttraction | Defined$ Opponent", "key:OpenAttraction.Defined")]
#[case::no_number("OpenAttraction | Amount$ X", "key:OpenAttraction.Amount")]
#[case::another_die(
    "RollDice | Sides$ 20 | ToVisitYourAttractions$ True",
    "key:RollDice.ToVisitYourAttractions-sides"
)]
fn near_misses_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}

#[test]
fn the_visit_keyword_is_the_visit_ability() {
    let expected = normal(json!({"Triggered": {"when": {"Visit": {}}, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&attraction("Draw | NumCards$ 1")), vec![expected]);
}

#[test]
fn a_visit_keyword_off_an_attraction_stays_a_gap() {
    let card = "Name:C\nTypes:Creature Elf\nPT:1/1\nK:Visit:Go\nSVar:Go:DB$ Draw | NumCards$ 1\n";
    assert_eq!(gaps(card), vec!["keyword:Visit-shape".to_owned()]);
}

#[rstest]
#[case::you_control("Attraction.YouCtrl", None)]
#[case::anyone("Attraction", Some("zone:Trigger.Origin=AttractionDeck"))]
fn the_open_trigger(#[case] valid: &str, #[case] gap: Option<&str>) {
    let card = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ ChangesZone | Origin$ AttractionDeck | Destination$ Battlefield | ValidCard$ {valid} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    );
    match gap {
        None => assert_eq!(
            abilities(&card),
            vec![normal(
                json!({"Triggered": {"when": {"OpenedAttraction": {"who": "You"}}, "effects": [{"Draw": {}}]}})
            )]
        ),
        Some(gap) => assert_eq!(gaps(&card), vec![gap.to_owned()]),
    }
}
