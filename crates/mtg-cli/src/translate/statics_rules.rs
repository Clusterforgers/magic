//! Static modes that change a rule of the game: what players and objects can't do, the hand size, the legend rule and
//! untapping during the other players' untap steps.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The rule of a static line this module reads; `None` for any other mode.
pub fn rule(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    if let Some(found) = super::statics_cant::rule(ctx, p) {
        return Some(found);
    }
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
        "Panharmonicon" => super::statics_again::again(ctx, p),
        "CombatDamageToughness" => toughness_damage(p),
        "CastWithFlash" => super::statics_cast::cast_with_flash(ctx, p),
        "CanAttackDefender" => {
            objects(ctx, p, mode, "ValidCard").map(|who| combat(&who, "AttackDespiteDefender"))
        }
        "BlockTapped" => {
            objects(ctx, p, mode, "ValidCard").map(|who| combat(&who, "BlockWhileTapped"))
        }
        "IgnoreHexproof" => ignore_hexproof(ctx, p),
        "IgnoreLegendRule" => ignore_legend_rule(p, mode),
        "UntapOtherPlayer" => untap_other_players(ctx, p),
        "Continuous" if p.has("SetMaxHandSize") => hand_size(ctx, p),
        "Continuous" if p.has("MayLookAt") && !p.has("MayPlay") => super::statics_look::look(p),
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
    keys(p, "CantTarget", &["ValidTarget", "ValidSource", "ValidSA"])?;
    let Some(target) = p.get("ValidTarget") else {
        return unmapped("key:Static.CantTarget.no-ValidTarget");
    };
    let who = match target {
        "You" | "Player" | "Opponent" | "Player.Opponent" => player_word(Some(target))?,
        word => object_word(ctx, word)?,
    };
    let mut sources = match p.get("ValidSource") {
        Some(valid) => selector_any(ctx, valid)?,
        None => Vec::new(),
    };
    match p.get("ValidSA") {
        None => {}
        Some("Spell") => sources.push(json!("Spell")),
        Some(_) => return unmapped("key:Static.CantTarget.ValidSA"),
    }
    Ok(cant(&who, &json!({"BeTargeted": sources})))
}

fn untap_other_players(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "UntapOtherPlayer", &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Permanent"))?;
    Ok(json!({"UntapDuring": {"what": what, "whose": "Opponents"}}))
}

/// A combat permission for the creatures `who` names.
fn combat(who: &Value, quirk: &str) -> Value {
    json!({"Combat": {"who": who, "quirk": quirk}})
}

/// `IgnoreHexproof`: spells and abilities the `Activator$` players control may target the `ValidEntity$` permanents and
/// players as though they did not have hexproof. A line that also frees its targets from ward is a gap.
fn ignore_hexproof(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "IgnoreHexproof", &["Activator", "ValidEntity"])?;
    if p.get("Description")
        .is_some_and(|text| text.contains("Ward"))
    {
        return unmapped("static:IgnoreHexproof-ward");
    }
    let by = match p.get("Activator") {
        None => json!("EachPlayer"),
        Some("You") => json!("You"),
        Some(other) => return unmapped(format!("key:Static.IgnoreHexproof.Activator={other}")),
    };
    let Some(entity) = p.get("ValidEntity") else {
        return unmapped("key:Static.IgnoreHexproof.no-ValidEntity");
    };
    let (mut permanents, mut players) = (Vec::new(), false);
    for word in entity.split(',') {
        match word {
            "Opponent" => players = true,
            other => permanents.extend(selector_any(ctx, other)?),
        }
    }
    let mut rule = json!({"by": by});
    if !permanents.is_empty() {
        rule["what"] = json!({"Each": permanents});
    }
    if players {
        rule["players"] = json!("Opponents");
    }
    Ok(json!({"IgnoreHexproof": rule}))
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

/// `CombatDamageToughness`: every creature of the players assigns combat damage equal to its toughness. A line that
/// limits it to some creatures (those with toughness above power, with defender) would change more than it prints.
fn toughness_damage(p: &Params) -> Tr<Value> {
    keys(p, "CombatDamageToughness", &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        Some("Creature.YouCtrl") => "You",
        Some("Creature") => "EachPlayer",
        Some(other) => {
            return unmapped(format!(
                "key:Static.CombatDamageToughness.ValidCard={other}"
            ));
        }
        None => return unmapped("key:Static.CombatDamageToughness.no-ValidCard"),
    };
    Ok(json!({"ToughnessAssignsDamage": who}))
}

/// `IgnoreLegendRule`: the legend rule stops for every player, or for the controller when `ValidCard$` is "permanents you
/// control". A narrower set of permanents (creatures, tokens, a name) is not a rule the engine has.
fn ignore_legend_rule(p: &Params, mode: &str) -> Tr<Value> {
    keys(p, mode, &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        None => "EachPlayer",
        Some("Permanent.YouCtrl" | "Card.YouCtrl") => "You",
        Some(_) => return unmapped("key:Static.IgnoreLegendRule.ValidCard"),
    };
    Ok(json!({"NoLegendRule": who}))
}
