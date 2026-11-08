//! Goal: lines the engine runs that the translator left as gaps translate to the rules the card prints, and the near-misses stay gaps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

use super::{abilities, gaps};

#[test]
fn a_flash_line_marked_defining_is_a_conditional_static_of_the_card_in_the_hand() {
    let text = "Name:Subject\nTypes:Creature Wurm\nPT:6/5\nS:Mode$ Continuous | CharacteristicDefining$ True | AddKeyword$ Flash | IsPresent$ Creature.YouCtrl | Description$ CARDNAME has flash as long as you control a creature.\n";
    let found = abilities(text);
    let rule = found
        .iter()
        .find_map(|ability| ability.get("Static"))
        .unwrap();
    assert_eq!(rule["from"], json!("Hand"));
    assert_eq!(rule["affected"], json!({"Each": [{"In": "Hand"}, "This"]}));
    assert_eq!(rule["mods"], json!([{"AddKeyword": "Flash"}]));
    assert!(rule.get("condition").is_some() && rule.get("defining").is_none());
}

#[test]
fn another_defining_keyword_stays_a_gap() {
    let text = "Name:Subject\nTypes:Creature Wurm\nPT:6/5\nS:Mode$ Continuous | CharacteristicDefining$ True | AddKeyword$ Haste | IsPresent$ Creature.YouCtrl | Description$ CARDNAME has haste as long as you control a creature.\n";
    assert!(gaps(text).iter().any(|gap| gap.contains("defining")));
}
