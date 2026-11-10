//! Rules of ep24 that are static lines: a draw limit (`Mode$ CantDraw | DrawLimit$ 1`, "Each opponent can't draw more
//! than one card each turn.") and opponents that can't make you sacrifice (`Mode$ CantSacrifice | ValidCause$
//! SpellAbility.OppCtrl`, "Spells and abilities your opponents control can't cause you to sacrifice permanents.").

use serde_json::{Value, json};

use super::ctx::{Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// `CantDraw` with `DrawLimit$`: the players draw no more than that many cards each turn.
pub fn ep24_draw_limit(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.CantDraw",
        &["Mode", "ValidPlayer", "DrawLimit"],
        |_, _| false,
    )?;
    let who = match p.get("ValidPlayer") {
        Some("Opponent") => "Opponents",
        Some("Player") => "EachPlayer",
        Some("You") => "You",
        _ => return unmapped("key:Static.CantDraw.ValidPlayer"),
    };
    let Some(most) = p.get("DrawLimit").and_then(|most| most.parse::<u32>().ok()) else {
        return unmapped("key:Static.CantDraw.DrawLimit");
    };
    Ok(json!({"DrawLimit": {"who": who, "most": most}}))
}

/// `Continuous | Affected$ You | AddKeyword$ PayLifeInsteadOf:B`: for each `{B}` in a cost you may pay 2 life rather
/// than that mana.
pub fn ep24_life_for_mana(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.Continuous",
        &["Mode", "Affected", "AddKeyword"],
        |_, _| false,
    )?;
    let color = match p
        .get("AddKeyword")
        .and_then(|k| k.strip_prefix("PayLifeInsteadOf:"))
    {
        Some("W") => "White",
        Some("U") => "Blue",
        Some("B") => "Black",
        Some("R") => "Red",
        Some("G") => "Green",
        _ => return unmapped("keyword:PayLifeInsteadOf"),
    };
    if p.get("Affected") != Some("You") {
        return unmapped("key:Static.Continuous.Affected");
    }
    Ok(json!({"PayLifeForMana": {"who": "You", "color": color}}))
}

/// `CantSacrifice` of your permanents caused by your opponents' spells and abilities, costs aside.
pub fn ep24_spared_sacrifice(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.CantSacrifice",
        &["Mode", "ValidCard", "ValidCause", "ForCost"],
        |_, _| false,
    )?;
    let shape = (p.get("ValidCard"), p.get("ValidCause"), p.get("ForCost"));
    if shape
        != (
            Some("Card.YouCtrl"),
            Some("SpellAbility.OppCtrl"),
            Some("False"),
        )
    {
        return unmapped("key:Static.CantSacrifice.ValidCause");
    }
    Ok(json!({"OpponentsCantMakeSacrifice": "You"}))
}
