//! A look at the top card that puts it onto the battlefield tapped if it's a land ("look at the top card of your
//! library. If it's a land card, you may put it onto the battlefield tapped"): `LookAtTop` of one card bound by a name,
//! then an `If` on that card being a land whose only step is a may-move of it.

use super::standin::with_effects;
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wording of an ability whose effects are the look and the land put onto the battlefield.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(Value::Array(effects)) = serde_json::to_value(ability.effects()) else {
        return;
    };
    if let [look, put] = effects.as_slice()
        && c10_cen_land_look(look, put).is_some()
    {
        texts.extend(with_effects(
            ability,
            "look at the top card of your library. If it's a land card, you may put it onto the battlefield tapped",
        ));
    }
}

/// Whether the two effects are the look at the top card and the may-put of it if it's a land.
fn c10_cen_land_look(look: &Value, put: &Value) -> Option<()> {
    let look = look.get("LookAtTop")?;
    let bound = look.get("as")?.as_str()?;
    if look.as_object()?.len() != 1 {
        return None;
    }
    let branch = put.get("If")?;
    let land = json!({"Matches": {"subject": {"Bound": bound}, "selector": [{"Type": "Land"}]}});
    if branch.as_object()?.len() != 2 || branch.get("condition")? != &land {
        return None;
    }
    let [may] = branch.get("then")?.as_array()?.as_slice() else {
        return None;
    };
    let may = may.get("May")?;
    let [moved] = may.get("effects")?.as_array()?.as_slice() else {
        return None;
    };
    let tapped =
        json!({"Move": {"what": {"Bound": bound}, "to": {"Battlefield": {"tapped": true}}}});
    (may.as_object()?.len() == 1 && *moved == tapped).then_some(())
}
