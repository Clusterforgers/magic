//! Static modes that change a rule of the game: what players and objects can't do, the hand size, the legend rule and
//! untapping during the other players' untap steps.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The rule of a static line this module reads; `None` for any other mode.
pub fn rule(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    let mode = p.get("Mode").unwrap_or_default();
    let found = match mode {
        "CantGainLife" => players(p, mode, "ValidPlayer").map(|who| json!({"CantGainLife": who})),
        "CantDraw" => players(p, mode, "ValidPlayer").map(|who| json!({"CantDraw": who})),
        "CantPlayLand" => players(p, mode, "Player").map(|who| cant(&who, &json!("PlayLand"))),
        "CantPutCounter" => put_counter(ctx, p),
        "CantSacrifice" => {
            objects(ctx, p, mode, "ValidCard").map(|who| cant(&who, &json!("Sacrifice")))
        }
        "CantTransform" => {
            objects(ctx, p, mode, "ValidCard").map(|who| cant(&who, &json!("Transform")))
        }
        "CantTarget" => cant_target(ctx, p),
        "IgnoreLegendRule" => keys(p, mode, &[]).map(|()| json!({"NoLegendRule": "EachPlayer"})),
        "UntapOtherPlayer" => untap_other_players(ctx, p),
        "Continuous" if p.has("SetMaxHandSize") => hand_size(ctx, p),
        _ => return None,
    };
    Some(found.map(|rule| json!({"Rule": {"rule": rule}})))
}

fn cant(who: &Value, action: &Value) -> Value {
    json!({"Cant": {"who": who, "action": action}})
}

/// Refuses a key the mode does not read; a static in another zone than the battlefield is another ability.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    check_keys(p, &format!("Static.{mode}"), read, |key, value| {
        key == "Mode" || (key == "EffectZone" && value == "Battlefield")
    })
}

/// The players a word names, as a reference.
fn player_word(word: Option<&str>) -> Tr<Value> {
    match word {
        None | Some("Player") => Ok(json!("EachPlayer")),
        Some("You") => Ok(json!("You")),
        Some("Opponent" | "Player.Opponent") => Ok(json!("Opponents")),
        Some(other) => unmapped(format!("static-affected:{other}")),
    }
}

fn players(p: &Params, mode: &str, key: &str) -> Tr<Value> {
    keys(p, mode, &[key])?;
    player_word(p.get(key))
}

/// The objects a valid-string names, as a reference.
fn object_word(ctx: &Ctx, word: &str) -> Tr<Value> {
    Ok(match word {
        "Card.Self" | "Creature.Self" | "Permanent.Self" => json!("This"),
        "Card.EnchantedBy" | "Creature.EnchantedBy" | "Card.EquippedBy" | "Creature.EquippedBy" => {
            json!("Attached")
        }
        other => json!({"Each": selector_any(ctx, other)?}),
    })
}

fn objects(ctx: &Ctx, p: &Params, mode: &str, key: &str) -> Tr<Value> {
    keys(p, mode, &[key])?;
    match p.get(key) {
        Some(word) => object_word(ctx, word),
        None => unmapped(format!("key:Static.{mode}.no-{key}")),
    }
}

/// `CantPutCounter` of any kind on objects, or on players.
fn put_counter(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantPutCounter", &["ValidCard", "ValidPlayer"])?;
    let who = match (p.get("ValidCard"), p.get("ValidPlayer")) {
        (Some(word), None) => object_word(ctx, word)?,
        (None, Some(word)) => player_word(Some(word))?,
        _ => return unmapped("key:Static.CantPutCounter.subject"),
    };
    Ok(cant(&who, &json!("PutCounter")))
}

/// `CantTarget`: the objects can't be the target of spells and abilities from the sources `ValidSource$` names.
fn cant_target(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantTarget", &["ValidTarget", "ValidSource"])?;
    let Some(target) = p.get("ValidTarget") else {
        return unmapped("key:Static.CantTarget.no-ValidTarget");
    };
    let who = match target {
        "You" | "Player" | "Opponent" | "Player.Opponent" => player_word(Some(target))?,
        word => object_word(ctx, word)?,
    };
    let sources = match p.get("ValidSource") {
        Some(valid) => selector_any(ctx, valid)?,
        None => Vec::new(),
    };
    Ok(cant(&who, &json!({"BeTargeted": sources})))
}

fn untap_other_players(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "UntapOtherPlayer", &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    Ok(json!({"UntapDuring": {"what": what, "whose": "Opponents"}}))
}

/// `SetMaxHandSize$` of the players `Affected$` names.
fn hand_size(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Continuous", &["Affected", "SetMaxHandSize"])?;
    let who = player_word(p.get("Affected"))?;
    match p.get("SetMaxHandSize") {
        Some("Unlimited") => Ok(json!({"NoMaxHandSize": who})),
        Some(size) => Ok(json!({"MaxHandSize": {"who": who, "size": amount(ctx, size)?}})),
        None => unmapped("key:Static.Continuous.SetMaxHandSize"),
    }
}
