//! `MayLookAt$`: looking at the top card of a library any time (`LookAtTopAnyTime`), playing with it revealed
//! (`RevealTop`) and looking at face-down cards in exile (`LookAtExiled`); the hand and face-down permanents stay gaps.

use serde_json::{Value, json};

use super::ctx::{Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The rule a `MayLookAt` static line stands for.
pub fn look(p: &Params) -> Tr<Value> {
    check_keys(
        p,
        "Static.MayLookAt",
        &["Affected", "AffectedZone", "MayLookAt"],
        |key, value| key == "Mode" || (key == "EffectZone" && value == "Battlefield"),
    )?;
    if p.get("AffectedZone") == Some("Exile") {
        return exiled(p, None);
    }
    if p.get("AffectedZone") != Some("Library") {
        return unmapped("static:MayLookAt-zone");
    }
    let who = match p.get("Affected").unwrap_or_default() {
        "Card.TopLibrary+YouCtrl" | "Card.TopLibrary+YouOwn" | "Card.YouOwn+TopLibrary" => "You",
        "Card.TopLibrary" => "EachPlayer",
        _ => return unmapped("static:MayLookAt-affected"),
    };
    // The seat that may look is the one named; "Player" means every seat sees the card.
    match (p.get("MayLookAt"), who) {
        (Some("You"), "You") => Ok(json!({"LookAtTopAnyTime": who})),
        (Some("Player"), _) => Ok(json!({"RevealTop": who})),
        _ => unmapped("static:MayLookAt-who"),
    }
}

/// The `LookAtExiled` rule of a `MayLookAt$` line about cards in exile: "you may look at the cards exiled with ~"
/// (`Card.ExiledWithSource`), or, with `only`, at the objects an `Effect` remembers (`Card.IsRemembered`).
pub fn exiled(p: &Params, only: Option<&Value>) -> Tr<Value> {
    let mut rule = match (p.get("Affected"), only) {
        (Some("Card.ExiledWithSource"), None) => {
            json!({"what": [{"ExiledWith": "This"}, {"In": "Exile"}]})
        }
        (Some("Card.IsRemembered"), Some(objects)) => {
            json!({"what": [{"In": "Exile"}], "only": objects})
        }
        _ => return unmapped("static:MayLookAt-exiled-affected"),
    };
    match p.get("MayLookAt") {
        Some("You") => {}
        Some("Player") => rule["who"] = json!("EachPlayer"),
        _ => return unmapped("static:MayLookAt-who"),
    }
    Ok(json!({ "LookAtExiled": rule }))
}
