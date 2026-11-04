//! Keywords whose Forge line carries more than a cost or a number: a color, a land type, a restriction, a deck rule or a
//! named part.

use mtg_cards::companion::DeckCondition;
use mtg_cards::keyword::Keyword;
use serde_json::{Value, json};

use super::cost;
use crate::translate::ctx::{Ctx, Tr, unmapped};
use crate::translate::valid::selector_any;

/// The keyword a line stands for when it is one of this file's; `None` for any other line.
pub fn special(ctx: &Ctx, text: &str) -> Option<Tr<Value>> {
    if let Some(color) = text.strip_prefix("Protection from ") {
        return protection_from(color);
    }
    let fields: Vec<&str> = text.split(':').collect();
    Some(match fields.as_slice() {
        ["Affinity", valid] | ["Affinity", valid, _] => affinity(ctx, valid),
        ["TypeCycling", valid, price] | ["TypeCycling", valid, price, _] => {
            typecycling(ctx, valid, price)
        }
        ["Gift"] => gift(ctx),
        ["Backup", count, part] => backup(ctx, count, part),
        ["Station", number] => station(number),
        ["Equip", price, valid] | ["Equip", price, valid, _] if !valid.is_empty() => {
            equip_to(ctx, price, valid)
        }
        ["Companion", ..] => companion(text),
        _ => return None,
    })
}

/// `Protection from black`: a color. Any other quality stays a gap.
fn protection_from(color: &str) -> Option<Tr<Value>> {
    let name = match color {
        "white" => "White",
        "blue" => "Blue",
        "black" => "Black",
        "red" => "Red",
        "green" => "Green",
        _ => return None,
    };
    Some(Ok(json!({"Protection": [{"Color": name}]})))
}

fn affinity(ctx: &Ctx, valid: &str) -> Tr<Value> {
    Ok(json!({"Affinity": selector_any(ctx, valid)?}))
}

/// `TypeCycling:Basic:2`: the cards searched for are the basic lands, a land type or another type or subtype.
fn typecycling(ctx: &Ctx, valid: &str, price: &str) -> Tr<Value> {
    let find = match valid {
        "Basic" => vec![json!({"Type": "Land"}), json!({"Supertype": "Basic"})],
        "Land" => vec![json!({"Type": "Land"})],
        other => selector_any(ctx, other)?,
    };
    Ok(json!({"Typecycling": {"find": find, "cost": cost(ctx, price)?}}))
}

/// `Gift`: the gift the card's `GiftAbility` part describes in words.
fn gift(ctx: &Ctx) -> Tr<Value> {
    let Some(text) = ctx.svar("GiftAbility") else {
        return unmapped("svar:missing");
    };
    let params = crate::forge::script::Params::parse(text);
    let kind = match params.get("GiftDescription") {
        Some("a card") => "Card",
        Some("a Treasure") => "Treasure",
        Some("a Food") => "Food",
        Some("a tapped Fish") => "TappedFish",
        Some("an Octopus") => "Octopus",
        Some("an extra turn") => "ExtraTurn",
        _ => return unmapped("keyword:Gift-kind"),
    };
    Ok(json!({ "Gift": kind }))
}

/// `Backup:1:BackupAbility`: the abilities after the keyword are the card's own, set by the compiler.
fn backup(ctx: &Ctx, count: &str, part: &str) -> Tr<Value> {
    match count.parse::<u32>() {
        Ok(count) if count > 0 && ctx.svar(part).is_some() => {
            Ok(json!({"Backup": {"count": count}}))
        }
        _ => unmapped("keyword:Backup-shape"),
    }
}

/// `Station:8`: the threshold is also on the card's `STATION` lines, which carry it.
fn station(number: &str) -> Tr<Value> {
    match number.parse::<u32>() {
        Ok(_) => Ok(json!("Station")),
        Err(_) => unmapped("keyword:Station-shape"),
    }
}

/// `Equip:3:Creature.YouCtrl+Legendary:legendary creature`: the creature must also be legendary, a commander, a token or
/// of a creature type; a flavor name restricts nothing; any other restriction stays a gap.
fn equip_to(ctx: &Ctx, price: &str, valid: &str) -> Tr<Value> {
    // A flavor name labels the ability and restricts nothing.
    if valid.starts_with("Flavor ") {
        return Ok(json!({"Equip": cost(ctx, price)?}));
    }
    // "Elf.YouCtrl" is a creature of that type you control.
    let spelled = match valid.split_once('.') {
        Some((word, rest)) if word != "Creature" && ctx.world.subtypes.contains(word) => {
            Some(format!("Creature.{word}+{rest}"))
        }
        _ => None,
    };
    let valid = spelled.as_deref().unwrap_or(valid);
    // Alternatives ("Shaman, Warlock, or Wizard") would need an either-or of selectors.
    if valid.contains(',') {
        return unmapped("keyword:Equip-with-restriction");
    }
    let Some(rest) = valid.strip_prefix("Creature.") else {
        return unmapped("keyword:Equip-with-restriction");
    };
    let rest: Vec<&str> = rest.split('+').filter(|word| *word != "YouCtrl").collect();
    let preds = selector_any(ctx, &format!("Creature.{}", rest.join("+")))?;
    let only: Vec<Value> = preds
        .into_iter()
        .filter(|pred| *pred != json!({"Type": "Creature"}))
        .collect();
    let named = match only.as_slice() {
        [one] => {
            *one == json!({"Supertype": "Legendary"})
                || *one == json!("Commander")
                || *one == json!("Token")
                || one.get("Subtype").is_some()
        }
        _ => false,
    };
    if !named {
        return unmapped("keyword:Equip-with-restriction");
    }
    Ok(json!({"EquipTo": {"to": only, "cost": cost(ctx, price)?}}))
}

/// `Companion:...`: the deck rules that read from the cards alone; the others stay gaps.
fn companion(text: &str) -> Tr<Value> {
    let value = |op: &str, to: u32| json!({"Compare": {"stat": "ManaValue", "op": op, "to": to}});
    let permanents: Vec<Value> = [
        "Artifact",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
        "Battle",
    ]
    .iter()
    .map(|ty| json!({ "Type": ty }))
    .collect();
    let condition = if text.starts_with("Companion:Special:UniqueNames:") {
        json!({"Distinct": {"of": [{"Not": {"Type": "Land"}}]}})
    } else if text.starts_with("Companion:Special:DeckSizePlus20:") {
        json!({"ExtraCards": 20})
    } else if text.starts_with("Companion:Permanent.cmcLE2,Instant,Sorcery:") {
        json!({"Each": {"of": [{"AnyOf": permanents}], "has": [value("Le", 2)]}})
    } else if text.starts_with("Companion:Card.cmcGE3,Land:") {
        json!({"Each": {"has": [{"AnyOf": [{"Type": "Land"}, value("Ge", 3)]}]}})
    } else {
        return unmapped("keyword:Companion");
    };
    let Ok(condition) = serde_json::from_value::<DeckCondition>(condition) else {
        return unmapped("keyword:Companion");
    };
    serde_json::to_value(Keyword::companion(condition)).or_else(|_| unmapped("keyword:Companion"))
}
