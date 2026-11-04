//! Properties of a valid-string that the DSL's selector atoms stand for: how an object was cast, where it sits in a
//! library, who blocks it and what happened to it this turn.

use serde_json::{Value, json};

/// The predicates of a property such as `wasCastFromYourHandByYou` or `blockingSource`, when it is one of these.
pub fn atom(prop: &str) -> Option<Vec<Value>> {
    let one = |value: Value| Some(vec![value]);
    match prop {
        "TopLibrary" => return one(json!("TopOfLibrary")),
        "BottomLibrary" => return one(json!("BottomOfLibrary")),
        "Suspended" => return one(json!("Suspended")),
        "ThisTurnEntered" => return one(json!("EnteredThisTurn")),
        "wasDealtDamageThisTurn" => return one(json!("DamagedThisTurn")),
        "blockingSource" => return one(json!({"Blocks": "This"})),
        "blockedBySource" => return one(json!({"BlockedBy": "This"})),
        "attackingYou" => return one(json!({"AttackingPlayer": "You"})),
        _ => {}
    }
    was_cast(prop.strip_prefix("wasCast")?)
}

/// `wasCast`, `wasCastByYou`, `wasCastFromYourHand`, `wasCastFromGraveyardByYou` and their kin.
fn was_cast(rest: &str) -> Option<Vec<Value>> {
    let (rest, by) = match rest.strip_suffix("ByYou") {
        Some(rest) => (rest, Some(json!("You"))),
        None => (rest, None),
    };
    let mut cast = serde_json::Map::new();
    if let Some(by) = by {
        cast.insert("by".to_owned(), by);
    }
    let mut owner = false;
    if let Some(zone) = rest.strip_prefix("From") {
        let (zone, yours) = match zone.strip_prefix("Your") {
            Some(zone) => (zone, true),
            None => (zone, false),
        };
        if !matches!(zone, "Hand" | "Graveyard" | "Exile" | "Library") {
            return None;
        }
        cast.insert("from".to_owned(), json!(zone));
        owner = yours;
    } else if !rest.is_empty() {
        return None;
    }
    let mut preds = vec![json!({ "WasCast": cast })];
    if owner {
        preds.push(json!({"OwnedBy": "You"}));
    }
    Some(preds)
}
