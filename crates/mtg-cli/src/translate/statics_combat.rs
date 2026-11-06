//! Static modes of combat: attack taxes, damage that can't be prevented and limits on the creatures that block.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The ability of a static line of one of these modes; `None` for any other.
pub fn ability(ctx: &Ctx, p: &Params) -> Option<Tr<Value>> {
    Some(match p.get("Mode").unwrap_or_default() {
        "MinMaxBlocker" => block_limit(ctx, p),
        "CantAttackUnless" => attack_tax(ctx, p).map(|found| json!({"Rule": {"rule": found}})),
        "CantPreventDamage" => no_prevention(ctx, p).map(|found| json!({"Rule": {"rule": found}})),
        _ => return None,
    })
}

/// Refuses a key the mode does not read.
fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    check_keys(p, &format!("Static.{mode}"), read, |key, value| {
        key == "Mode" || (key == "EffectZone" && value == "Battlefield")
    })
}

/// `MinMaxBlocker`: the creatures `ValidCard$` names can't be blocked by more than `Max$` creatures, or except by `Min$`
/// or more (CR 509.1b).
fn block_limit(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "MinMaxBlocker", &["ValidCard", "Min", "Max"])?;
    let number = |key: &str| match p.get(key) {
        None => Ok(None),
        Some(text) => text
            .parse::<u32>()
            .map(Some)
            .or_else(|_| unmapped(format!("key:Static.MinMaxBlocker.{key}={text}"))),
    };
    let modification = match (number("Min")?, number("Max")?) {
        (Some(least), None) if least > 1 => json!({"MinBlockers": least}),
        (None, Some(most)) if most > 0 => json!({"MaxBlockers": most}),
        _ => return unmapped("static:MinMaxBlocker-shape"),
    };
    let affected = match p.get("ValidCard").unwrap_or_default() {
        "Card.Self" | "Creature.Self" => json!("This"),
        "Creature.EnchantedBy" | "Creature.EquippedBy" => json!("Attached"),
        valid @ ("" | "Card.EffectSource" | "Card.IsRemembered") => {
            return unmapped(format!("key:Static.MinMaxBlocker.ValidCard={valid}"));
        }
        valid => json!({"Each": selector_any(ctx, valid)?}),
    };
    Ok(json!({"Static": {"affected": affected, "mods": [modification]}}))
}

/// `CantAttackUnless` with a payment of mana or life: creatures can't attack the players or planeswalkers `Target$` names
/// unless their controller pays for each of them (CR 508.1h).
fn attack_tax(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantAttackUnless", &["ValidCard", "Target", "Cost"])?;
    if !p.has("Target") {
        let creature = match p.get("ValidCard") {
            Some("Card.Self" | "Creature.Self") => Some("This"),
            Some("Creature.AttachedBy" | "Creature.EnchantedBy" | "Creature.EquippedBy") => {
                Some("Attached")
            }
            _ => None,
        };
        if let Some(creature) = creature {
            return self_tax(ctx, p, creature);
        }
    }
    let attacked = match p.get("Target") {
        Some("You") => "Player",
        Some("Planeswalker.YouCtrl") => "Planeswalkers",
        Some("You,Planeswalker.YouCtrl") => "Both",
        _ => return unmapped("key:Static.CantAttackUnless.Target"),
    };
    let mut tax = json!({"attacked": attacked});
    match p.get("Cost").unwrap_or_default() {
        "PayLife<2>" => tax["life"] = json!(2),
        "WP" => tax["mana"] = json!("{W/P}"),
        digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
            tax["mana"] = json!(format!("{{{digits}}}"));
        }
        other => return unmapped(format!("cost:Static.CantAttackUnless.{other}")),
    }
    match p.get("ValidCard").unwrap_or_default() {
        "Creature" => {}
        valid if valid.contains("Self") || valid.contains("AttachedBy") => {
            return unmapped("key:Static.CantAttackUnless.ValidCard-of-itself");
        }
        valid => tax["attackers"] = json!(selector_any(ctx, valid)?),
    }
    Ok(json!({"AttackTax": tax}))
}

/// `CantAttackUnless` on the card itself or the creature it is attached to (`creature` is its reference), with no player
/// named: the creature costs the mana, once or for each of the amount an SVar counts, as it attacks anyone (CR 508.1h).
fn self_tax(ctx: &Ctx, p: &Params, creature: &str) -> Tr<Value> {
    let cost = p.get("Cost").unwrap_or_default();
    let (mana, per) = if !cost.is_empty() && cost.bytes().all(|b| b.is_ascii_digit()) {
        (format!("{{{cost}}}"), json!(1))
    } else if ctx.svar(cost).is_some() {
        ("{1}".to_owned(), super::amount::amount(ctx, cost)?)
    } else {
        return unmapped(format!("cost:Static.CantAttackUnless.{cost}"));
    };
    Ok(json!({"AttackTax": {
        "who": "EachPlayer", "attacked": "Both", "attackers": [{"Is": creature}], "mana": mana, "per": per}}))
}

/// `CantPreventDamage`: damage from the sources `ValidSource$` names (all damage, or combat damage with `IsCombat$`)
/// can't be prevented (CR 614.17).
fn no_prevention(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "CantPreventDamage", &["ValidSource", "IsCombat"])?;
    let mut rule = json!({});
    if let Some(valid) = p.get("ValidSource") {
        rule["source"] = json!(selector_any(ctx, valid)?);
    }
    match p.get("IsCombat") {
        None => {}
        Some("True") => rule["combat"] = json!(true),
        Some(other) => return unmapped(format!("key:Static.CantPreventDamage.IsCombat={other}")),
    }
    Ok(json!({"DamageCantBePrevented": rule}))
}

const BLOCKED_BY_ALL: &str = "All creatures able to block CARDNAME do so";
const PREVENT_ALL: &str = "Prevent all damage that would be dealt to CARDNAME";

fn rule_word(word: &str) -> &str {
    let word = word.trim_end_matches('.');
    word.strip_prefix("HIDDEN ").unwrap_or(word)
}

/// Whether a keyword of a pump line is a rule about the object and no change to it.
pub fn is_rule_word(word: &str) -> bool {
    let word = rule_word(word);
    word == "CARDNAME must be blocked if able" || word == BLOCKED_BY_ALL || word == PREVENT_ALL
}

/// The effect of a keyword of a pump line that `is_rule_word`, for the objects `affected` names, for the duration: the
/// attackers must be blocked (CR 509.1c) or all damage to them is prevented this turn (CR 615).
pub fn hidden_effect(word: &str, affected: &Value, duration: &str) -> Tr<Value> {
    let word = rule_word(word);
    let combat = |quirk: Value| json!({"Rule": {"rule": {"Combat": {"who": affected, "quirk": quirk}}, "duration": duration}});
    if word == "CARDNAME must be blocked if able" {
        return Ok(combat(json!({"MustBeBlocked": "AtLeastOne"})));
    }
    if word == BLOCKED_BY_ALL {
        return Ok(combat(json!({"MustBeBlocked": "Every"})));
    }
    // A shield lasts the turn; a longer prevention is another rule.
    if duration != "UntilEndOfTurn" {
        return unmapped("keyword:Prevent-all-damage-for-longer");
    }
    Ok(
        json!({"Shield": {"when": {"Damage": {"to": affected}}, "by": "Prevent", "duration": duration}}),
    )
}

/// `CanBlockAmount$` and `CanBlockAny$`: the creature blocks more attackers each combat (CR 509.1a). "Any number" is more
/// attackers than a table has.
pub fn block_capacity(p: &Params) -> Tr<Vec<Value>> {
    let mut mods = Vec::new();
    match p.get("CanBlockAmount").map(str::parse::<u32>) {
        None => {}
        Some(Ok(count)) => mods.push(json!({"BlocksAdditional": count})),
        Some(Err(_)) => return unmapped("key:Static.Continuous.CanBlockAmount-count"),
    }
    match p.get("CanBlockAny") {
        None => {}
        Some("True") => mods.push(json!({"BlocksAdditional": ANY_NUMBER})),
        Some(_) => return unmapped("key:Static.Continuous.CanBlockAny"),
    }
    Ok(mods)
}

/// The extra blocks of "can block any number of creatures".
const ANY_NUMBER: u32 = 99;
