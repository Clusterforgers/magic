//! `CantAttack` with `UnlessDefender$`: the card attacks only a defending player for whom the condition holds.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_in;
use crate::forge::script::Params;

/// The rule of a `CantAttack` line with `UnlessDefender$`; `None` for a line without it.
pub fn rule(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    if p.get("Mode") != Some("CantAttack") {
        return None;
    }
    let word = p.get("UnlessDefender")?;
    Some(unless_defender(ctx, p, word))
}

fn unless_defender(ctx: &Ctx, p: &Params, word: &str) -> Tr<Value> {
    check_keys(
        p,
        "Static.CantAttack",
        &["ValidCard", "UnlessDefender"],
        |key, value| key == "Mode" || (key == "EffectZone" && value == "Battlefield"),
    )?;
    if p.get("ValidCard") != Some("Card.Self") {
        return unmapped("key:Static.CantAttack.UnlessDefender-ValidCard");
    }
    let unless = defender_condition(ctx, word)?;
    Ok(json!({"Rule": {"rule": {"Cant": {
        "who": "This",
        "action": {"Attack": {"unless": unless}},
    }}}}))
}

/// What holds for the defending player ("you" is that player): `controlsX` is a permanent of the kind on the
/// battlefield under their control, `!word` its negation.
fn defender_condition(ctx: &Ctx, word: &str) -> Tr<Value> {
    if let Some(inner) = word.strip_prefix('!') {
        return Ok(json!({"Not": defender_condition(ctx, inner)?}));
    }
    if word == "isMonarch" {
        return Ok(json!({"Monarch": "You"}));
    }
    let named = word
        .strip_prefix("controls")
        .filter(|kind| !kind.is_empty() && !kind.contains(','));
    let Some(kind) = named else {
        return unmapped(format!("key:Static.CantAttack.UnlessDefender={word}"));
    };
    // "Island" is "Island.YouCtrl"; "Creature.untapped" is "Creature.untapped+YouCtrl".
    let valid = if kind.contains('.') {
        format!("{kind}+YouCtrl")
    } else {
        format!("{kind}.YouCtrl")
    };
    let selector = selector_in(ctx, &valid, None)?;
    Ok(json!({"Compare": {"left": {"Count": selector}, "op": "Ge", "right": 1}}))
}
