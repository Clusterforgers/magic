//! Goal: the sticker and visit lines of Forge (`PutSticker` with the permanents a player owns, optional or not, and the `VisitAttraction` trigger) map to the DSL nodes the engine runs, and each near miss stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn spell(line: &str) -> String {
    format!("Name:S\nTypes:Sorcery\nA:SP$ {line}\n")
}

#[rstest]
#[case::nonland(
    "PutSticker | ValidCard$ Permanent.nonLand+YouOwn",
    json!([{"PutSticker": {"on": [{"Not": {"Type": "Land"}}, {"OwnedBy": "You"}]}}])
)]
#[case::optional(
    "PutSticker | ValidCard$ Creature.YouOwn | Optional$ True",
    json!([{"May": {"effects": [{"PutSticker": {"on": [{"Type": "Creature"}, {"OwnedBy": "You"}]}}]}}])
)]
fn the_sticker_verb_maps(#[case] line: &str, #[case] effects: Value) {
    let expected = normal(json!({"Spell": {"effects": effects}}));
    assert_eq!(abilities(&spell(line)), vec![expected]);
}

#[rstest]
#[case::another_player(
    "PutSticker | ValidCard$ Permanent.YouOwn | Defined$ Opponent",
    "key:PutSticker.Defined"
)]
#[case::no_permanent("PutSticker", "key:PutSticker.ValidCard")]
#[case::not_owned(
    "PutSticker | ValidCard$ Permanent.YouCtrl",
    "key:PutSticker.ValidCard-owner"
)]
#[case::odd_option(
    "PutSticker | ValidCard$ Permanent.YouOwn | Optional$ Maybe",
    "key:PutSticker.Optional"
)]
fn near_misses_stay_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), vec![gap.to_owned()]);
}

#[rstest]
#[case::you("ValidPlayer$ You", None)]
#[case::your_attraction("ValidCard$ Attraction.YouCtrl", None)]
#[case::anyone("ValidPlayer$ Player", Some("key:VisitAttraction.ValidPlayer"))]
fn the_visit_trigger(#[case] valid: &str, #[case] gap: Option<&str>) {
    let card = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nT:Mode$ VisitAttraction | {valid} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    );
    match gap {
        None => assert_eq!(
            abilities(&card),
            vec![normal(
                json!({"Triggered": {"when": {"VisitedAttraction": {"who": "You"}}, "effects": [{"Draw": {}}]}})
            )]
        ),
        Some(gap) => assert_eq!(gaps(&card), vec![gap.to_owned()]),
    }
}
