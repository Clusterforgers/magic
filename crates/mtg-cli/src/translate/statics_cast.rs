//! Static modes about casting: `CastWithFlash` ("you may cast creature spells as though they had flash") as a `MayCast`
//! permission, and `CantBeCast` for the players who can't cast any spell.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, is_hint, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The players `Caster$` names.
fn caster(word: Option<&str>) -> Tr<&'static str> {
    match word {
        Some("You") => Ok("You"),
        Some("Opponent") => Ok("Opponents"),
        None | Some("Player") => Ok("EachPlayer"),
        Some(other) => unmapped(format!("static-affected:Caster={other}")),
    }
}

/// Refuses a key the mode does not read; `EffectZone$ Command` is the zone of an `Effect` object.
fn only_keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    for (key, value) in p.iter() {
        match key {
            "Mode" => {}
            "EffectZone" if matches!(value, "Battlefield" | "Command") => {}
            _ if read.contains(&key) || is_hint(key) => {}
            _ => return unmapped(format!("key:Static.{mode}.{key}")),
        }
    }
    Ok(())
}

/// `CastWithFlash` of spells: the cards `ValidCard$` selects are cast as though they had flash by the `Caster$` players.
pub fn cast_with_flash(ctx: &Ctx, p: &Params) -> Tr<Value> {
    only_keys(p, "CastWithFlash", &["ValidCard", "ValidSA", "Caster"])?;
    if p.get("ValidSA") != Some("Spell") {
        return unmapped("static:CastWithFlash-not-a-spell");
    }
    let who = caster(p.get("Caster"))?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    if what
        .iter()
        .any(|pred| pred.to_string().contains("\"This\""))
    {
        return unmapped("static:CastWithFlash-the-card-itself");
    }
    let mut permission = json!({"flash": true});
    if who != "You" {
        permission["who"] = json!(who);
    }
    if !what.is_empty() {
        permission["what"] = json!(what);
    }
    Ok(json!({ "MayCast": permission }))
}

/// `CantBeCast` of every spell: the `Caster$` players cast none, always or only (with `Condition$ PlayerTurn`) on the
/// turns of the rule's controller. A limit on the cards or the number, and a spell's own casting condition, stay gaps.
pub fn cant_be_cast(p: &Params) -> Tr<Value> {
    only_keys(p, "CantBeCast", &["ValidCard", "Caster", "Condition"])?;
    if p.get("ValidCard") != Some("Card") {
        return unmapped("static:CantBeCast-of-some-cards");
    }
    Ok(json!({"CantCastSpells": {"who": caster(p.get("Caster"))?}}))
}
