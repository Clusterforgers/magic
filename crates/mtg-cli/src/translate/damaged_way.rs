//! "If a creature dealt damage this way would die this turn, exile it instead": `RememberDamaged$` with
//! `ReplaceDyingDefined$ Remembered` on a line that deals damage, read as a death shield over what the source damaged.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The `ReplaceDyingDefined$` word `split` leaves for the damage mapper: the objects this source dealt damage to.
pub const DAMAGED_THIS_WAY: &str = "DamagedThisWay";

/// The line without its `RememberDamaged$`, the shield named by [`DAMAGED_THIS_WAY`], when the line deals the damage
/// itself and no other line reads what it remembers; `None` leaves the line as it is.
pub fn split(ctx: &Ctx, p: &Params, api: &str) -> Option<Params> {
    let peek = |key: &str| p.iter().find(|(name, _)| *name == key).map(|(_, v)| v);
    if !matches!(api, "DealDamage" | "DamageAll")
        || peek("RememberDamaged") != Some("True")
        || !matches!(
            peek("ReplaceDyingDefined"),
            Some("Remembered" | "Remembered.Creature")
        )
        || !matches!(peek("DamageSource"), None | Some("Self"))
        || ctx.memory.crosses(super::memory::Slot::Remembered)
    {
        return None;
    }
    Some(
        p.without(&["RememberDamaged", "ReplaceDyingDefined"])
            .with("ReplaceDyingDefined", DAMAGED_THIS_WAY),
    )
}

/// The shield: what the source dealt damage to, as the card says it ("a creature", "a permanent"), goes to exile
/// instead of dying this turn. Another wording stays a gap by name.
pub fn shield(ctx: &Ctx) -> Tr<Value> {
    let text = ctx.oracle.to_lowercase();
    let mut what = Vec::new();
    if text.contains("if a creature dealt damage this way would die this turn, exile it instead") {
        what.push(json!({"Type": "Creature"}));
    } else if !text
        .contains("if a permanent dealt damage this way would die this turn, exile it instead")
    {
        return unmapped("key:ReplaceDyingDefined=Remembered-other-words");
    }
    what.push(json!({"DamagedBy": "This"}));
    Ok(json!({"Shield": {
        "when": {"ZoneChange": {"what": what, "from": "Battlefield", "to": "Graveyard"}},
        "by": {"MoveTo": "Exile"},
        "duration": "UntilEndOfTurn"}}))
}
