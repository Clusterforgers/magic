//! The hand ability of the New Capenna hideaway creatures, worded as the card prints it: "{2}, Exile ~ from your hand:
//! Target land gains "{T}: Add {U}, {B}, or {R}" until ~ is cast from exile. You may cast ~ for as long as it remains
//! exiled." The ability must be that shape whole: a mana cost and the exile of the source, a target land, the grant until
//! the card is cast from exile and the permission that names the exiled card alone.

use super::super::variants;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::Effect;
use crate::modify::Modification;

/// The ability of that shape for the mana cost and the granted ability, as JSON.
fn shape(mana: &str, granted: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({"Activated": {
        "cost": [{"Mana": mana}, "ExileSelf"],
        "from": "Hand",
        "targets": [{"what": {"Object": [{"Type": "Land"}]}}],
        "effects": [
            {"Continuous": {"affected": {"Target": 0}, "mods": [{"GrantAbility": granted}], "duration": "UntilCastFromExile"}},
            {"Rule": {"rule": {"MayCast": {"only": {"Arrived": {"of": "This", "zone": "Exile"}}, "from": "Exile"}}, "duration": "Forever"}}
        ]
    }})
}

/// Adds the wordings of the hand ability, naming the card `~` or "this card".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { cost, effects, .. } = ability else {
        return;
    };
    let [CostItem::Mana(mana), CostItem::ExileSelf] = cost.0.as_slice() else {
        return;
    };
    let [Effect::Continuous { mods, .. }, _] = effects.as_slice() else {
        return;
    };
    let [Modification::GrantAbility(granted)] = mods.as_slice() else {
        return;
    };
    let Ok(granted_json) = serde_json::to_value(&**granted) else {
        return;
    };
    let mana = mana.to_string();
    let same = serde_json::from_value::<Ability>(shape(&mana, &granted_json))
        .is_ok_and(|expected| expected == *ability);
    if !same {
        return;
    }
    for gained in variants(granted) {
        let gained = gained.trim_end_matches('.');
        for name in ["~", "this card"] {
            let text = format!(
                "{mana}, Exile {name} from your hand: Target land gains \"{gained}\" until {name} is cast from exile. You may cast {name} for as long as it remains exiled."
            );
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}
