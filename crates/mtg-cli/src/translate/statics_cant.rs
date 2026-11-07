//! Static modes whose subject is more than a `Ref`: `CantBeCast`, `CantBeActivated` and `DisableTriggers` become
//! `GameRule::Cant` with a spell filter, a source filter or a trigger pattern.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The rule of a static line of one of these modes; `None` for any other.
pub fn rule(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    rule_for(ctx, p, None)
}

/// [`rule`], where `Player.IsRemembered` names the players `remembered` stands for (an `Effect` object that remembers a
/// player: "target player can't cast spells this turn").
pub fn rule_for(ctx: &Ctx, p: &Params, remembered: Option<&Value>) -> Option<Tr<Value>> {
    let found = match p.get("Mode").unwrap_or_default() {
        "CantBeCast" => cant_be_cast(ctx, p, remembered),
        "CantBeActivated" => cant_be_activated(ctx, p, remembered),
        "DisableTriggers" => disable_triggers(ctx, p),
        _ => return None,
    };
    Some(found)
}

/// A static line of the mode `CantAttack,CantBlock,CantBeActivated` ("can't attack or block and its activated abilities can't
/// be activated") as the two lines of its parts; any other line stays whole.
pub fn split_modes(body: &str) -> Vec<String> {
    const MODE: &str = "Mode$ CantAttack,CantBlock,CantBeActivated";
    if !body.contains(MODE) || body.contains("Secondary$") {
        return vec![body.to_owned()];
    }
    // Oracle bans mana abilities too unless the line says it does not (`ValidSA$ Activated.!ManaAbility`), and a
    // `ValidSA$` of its own stays as written.
    let activated = if body.contains("ValidSA$") {
        "Mode$ CantBeActivated"
    } else {
        "Mode$ CantBeActivated | ValidSA$ Activated"
    };
    vec![
        // The attack and block line has no ability to qualify.
        body.replacen(MODE, "Mode$ CantAttack,CantBlock", 1)
            .split(" | ")
            .filter(|part| !part.starts_with("ValidSA$"))
            .collect::<Vec<_>>()
            .join(" | "),
        body.replacen(MODE, activated, 1),
    ]
}

/// Refuses a key the mode does not read; a static in another zone than the battlefield is another ability.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    check_keys(p, &format!("Static.{mode}"), read, |key, value| {
        key == "Mode" || (key == "EffectZone" && value == "Battlefield")
    })
}

/// The players a `Caster$` or `Activator$` word names; every player when absent.
fn who(word: Option<&str>, remembered: Option<&Value>) -> Tr<Value> {
    match word {
        Some("Player.IsRemembered") if remembered.is_some() => {
            Ok(remembered.cloned().unwrap_or_default())
        }
        None | Some("Player") => Ok(json!("EachPlayer")),
        Some("You") => Ok(json!("You")),
        Some("Opponent") => Ok(json!("Opponents")),
        Some(other) => super::players_where::word(other)
            .map_or_else(|| unmapped(format!("static-affected:{other}")), Ok),
    }
}

fn cant(who: &Value, action: &Value) -> Value {
    json!({"Rule": {"rule": {"Cant": {"who": who, "action": action}}}})
}

/// `CantBeCast`: the spells `ValidCard$` names can't be cast by the `Caster$`, once `NumLimitEachTurn$` of them were.
fn cant_be_cast(ctx: &Ctx, p: &Params, remembered: Option<&Value>) -> Tr<Value> {
    keys(
        p,
        "CantBeCast",
        &["ValidCard", "Caster", "NumLimitEachTurn"],
    )?;
    let filter = match p.get("ValidCard") {
        None | Some("Card") => Vec::new(),
        Some(valid) => selector_any(ctx, valid)?,
    };
    let mut action = json!({});
    if !filter.is_empty() {
        action["filter"] = json!(filter);
    }
    if let Some(limit) = p.get("NumLimitEachTurn") {
        let Ok(limit) = limit.parse::<u32>() else {
            return unmapped("key:Static.CantBeCast.NumLimitEachTurn-not-a-number");
        };
        action["more_than"] = json!(limit);
    }
    Ok(cant(
        &who(p.get("Caster"), remembered)?,
        &json!({"Cast": action}),
    ))
}

/// `CantBeActivated`: the activated abilities (not the mana abilities) of the permanents `ValidCard$` names can't be
/// activated, by the `Activator$` when it names one.
fn cant_be_activated(ctx: &Ctx, p: &Params, remembered: Option<&Value>) -> Tr<Value> {
    keys(
        p,
        "CantBeActivated",
        &["ValidCard", "ValidSA", "Activator", "AffectedZone"],
    )?;
    if !matches!(p.get("AffectedZone"), None | Some("Battlefield")) {
        return unmapped("key:Static.CantBeActivated.AffectedZone");
    }
    // A loyalty ability and the cycling ability are told apart by their kind; every activated ability but the mana
    // abilities is the plain form (an unqualified `Activated` includes the mana abilities, which no rule here forbids).
    let kind = match p.get("ValidSA") {
        Some("Activated.!ManaAbility") => None,
        // Mana abilities are activated abilities too (CR 605): they are forbidden with the rest.
        None | Some("Activated") => Some("Every"),
        Some("Activated.Loyalty") => Some("Loyalty"),
        Some("Activated.Cycling") => Some("Cycling"),
        _ => return unmapped("key:Static.CantBeActivated.ValidSA"),
    };
    let sources = match (p.get("ValidCard"), kind) {
        (Some(valid), _) => selector_any(ctx, valid)?,
        (None, Some("Loyalty" | "Cycling")) => Vec::new(),
        (None, _) => return unmapped("key:Static.CantBeActivated.no-ValidCard"),
    };
    if let Some(kind) = kind {
        let action = json!({"ActivateKind": {"kind": kind, "sources": sources}});
        return Ok(cant(&who(p.get("Activator"), remembered)?, &action));
    }
    match p.get("Activator") {
        None => Ok(cant(&json!({"Each": sources}), &json!({"Activate": []}))),
        Some(word) => Ok(cant(
            &who(Some(word), remembered)?,
            &json!({"Activate": sources}),
        )),
    }
}

/// `DisableTriggers`: permanents entering or leaving don't cause the abilities of the permanents to trigger.
fn disable_triggers(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "DisableTriggers",
        &[
            "ValidCause",
            "ValidMode",
            "Origin",
            "Destination",
            "ValidCard",
        ],
    )?;
    if p.get("ValidMode") != Some("ChangesZone,ChangesZoneAll") {
        return unmapped("key:Static.DisableTriggers.ValidMode");
    }
    let Some(cause) = p.get("ValidCause") else {
        return unmapped("key:Static.DisableTriggers.no-ValidCause");
    };
    let mut pattern = json!({"what": selector_any(ctx, cause)?});
    for (key, field) in [("Origin", "from"), ("Destination", "to")] {
        match p.get(key) {
            None => {}
            Some(zone @ ("Battlefield" | "Graveyard" | "Hand" | "Library" | "Exile")) => {
                pattern[field] = json!(zone);
            }
            Some(_) => return unmapped(format!("zone:Static.DisableTriggers.{key}")),
        }
    }
    let holders = match p.get("ValidCard") {
        None => json!("EachPlayer"),
        Some("Permanent.OppCtrl+inZoneBattlefield") => json!("Opponents"),
        Some(_) => return unmapped("key:Static.DisableTriggers.ValidCard"),
    };
    Ok(cant(&holders, &json!({"Trigger": {"ZoneChange": pattern}})))
}
