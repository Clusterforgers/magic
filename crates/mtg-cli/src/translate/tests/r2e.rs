//! Goal: round 2 pack E (mana riders, mana kinds, small costs) translates each construct to the rule it names, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn trigger_card(lines: &str) -> String {
    format!("Name:T\nTypes:Enchantment\n{lines}\n")
}

#[rstest]
#[case::mandatory_sacrifice(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | Execute$ TrigSac\nSVar:TrigSac:AB$ Draw | NumCards$ 3 | Cost$ Mandatory Sac<1/CARDNAME>",
    json!([{"Sacrifice": {"filter": ["This"]}}, {"If": {"condition": "Performed", "then": [{"Draw": {"count": 3}}]}}])
)]
#[case::dies_trigger_token(
    "T:Mode$ ChangesZone | ValidCard$ Creature.YouCtrl | Origin$ Battlefield | Destination$ Graveyard | Execute$ TrigSac | TriggerZones$ Battlefield | TriggerDescription$ When a creature you control dies, sacrifice CARDNAME. If you do, create four 1/1 colorless Spirit creature tokens.\nSVar:TrigSac:AB$ Token | TokenAmount$ 4 | TokenScript$ w_1_1_soldier | TokenOwner$ You | Cost$ Mandatory Sac<1/CARDNAME>",
    json!([])
)]
fn a_cost_in_a_resolving_part_becomes_its_effects(#[case] lines: &str, #[case] effects: Value) {
    let text = trigger_card(lines);
    let found = abilities(&text);
    assert_eq!(found[0]["Triggered"]["effects"], effects, "{:?}", gaps(&text));
}
