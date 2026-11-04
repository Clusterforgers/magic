//! `MayLookAt$`: looking at the top card of a library any time (`LookAtTopAnyTime`) and playing with it revealed
//! (`RevealTop`); the hand, face-down permanents and exiled cards stay gaps.

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
