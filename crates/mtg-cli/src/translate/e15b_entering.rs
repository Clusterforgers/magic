//! What the card itself does as it enters: the day it brings with it (an `Event$ Moved` line) and the choice of a card
//! type or a land type (an `ETBReplacement` part).

use mtg_core::card::{CardType, CardTypes};
use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// "If it's neither day nor night, it becomes day as ~ enters" (`DayTime$ Neither` and a `DayTime` part that makes it
/// day); `None` for a line without `DayTime$`.
pub fn day_if_neither(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Option<Tr<Value>> {
    let when = p.get("DayTime")?;
    Some(day(ctx, p, when, condition))
}

fn day(ctx: &mut Ctx, p: &Params, when: &str, condition: Option<&Value>) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.Moved",
        &[
            "Event",
            "ValidCard",
            "Destination",
            "DayTime",
            "ReplaceWith",
            "ReplacementResult",
        ],
        |_, _| false,
    )?;
    let shape = when == "Neither"
        && condition.is_none()
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("Destination") == Some("Battlefield")
        && matches!(p.get("ReplacementResult"), None | Some("Updated"));
    if !shape {
        return unmapped("key:Replacement.Moved.DayTime");
    }
    let part = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    check_keys(&part, "Replacement", &["DB", "Value"], |_, _| false)?;
    if part.get("DB") != Some("DayTime") || part.get("Value") != Some("Day") {
        return unmapped("key:Replacement.Moved.DayTime");
    }
    Ok(json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "DayIfNeither"}}))
}

/// The card types `ChooseType` may name, with the engine's type of each.
const CARD_TYPES: [(&str, CardType); 9] = [
    ("Artifact", CardType::Artifact),
    ("Battle", CardType::Battle),
    ("Creature", CardType::Creature),
    ("Enchantment", CardType::Enchantment),
    ("Instant", CardType::Instant),
    ("Kindred", CardType::Kindred),
    ("Land", CardType::Land),
    ("Planeswalker", CardType::Planeswalker),
    ("Sorcery", CardType::Sorcery),
];

/// The `ETBReplacement` part `DB$ ChooseType` of a card type or a land type: "choose a card type", "choose artifact,
/// creature, enchantment, instant, or sorcery" (`ValidTypes$`), "a card type other than creature or land"
/// (`InvalidTypes$`), "choose a land type" and "choose a basic land type". A creature type is read elsewhere.
pub fn type_choice(part: &Params) -> Tr<Value> {
    check_keys(
        part,
        "ETBReplacement",
        &["DB", "Defined", "Type", "ValidTypes", "InvalidTypes"],
        |_, _| false,
    )?;
    let words = |key: &str| {
        part.get(key)
            .map(|list| list.split(',').map(str::trim).collect::<Vec<_>>())
    };
    let known = |list: &[&str]| {
        !list.is_empty()
            && list
                .iter()
                .all(|word| CARD_TYPES.iter().any(|(name, _)| name == word))
    };
    let kind = match (part.get("Type"), words("ValidTypes"), words("InvalidTypes")) {
        (Some("Card"), None, None) => json!("CardType"),
        (Some("Card"), Some(valid), None) if valid.len() >= 2 && known(&valid) => {
            card_types_of(|word| valid.contains(&word))
        }
        (Some("Card"), None, Some(invalid)) if known(&invalid) => {
            card_types_of(|word| !invalid.contains(&word))
        }
        (Some("Basic Land"), None, None) => json!("BasicLandType"),
        (Some("Land"), None, None) => json!("LandType"),
        (Some("Card" | "Basic Land" | "Land"), _, _) => {
            return unmapped("key:ETBReplacement.ValidTypes");
        }
        _ => return unmapped("keyword:ETBReplacement-ChooseType"),
    };
    Ok(json!({ "Choose": kind }))
}

/// `{"CardTypeOf": n}`: the card types the test keeps, as the engine's set.
fn card_types_of(keep: impl Fn(&str) -> bool) -> Value {
    let set: CardTypes = CARD_TYPES
        .iter()
        .filter(|(word, _)| keep(word))
        .map(|(_, kind)| *kind)
        .collect();
    json!({ "CardTypeOf": set })
}

/// The `ETBReplacement` part `DB$ PutCounter | CounterType$ Deathtouch,Lifelink`: "enters with your choice of a
/// deathtouch counter or a lifelink counter on it". One counter, of two or more kinds; any other shape stays a gap.
pub fn counter_of(part: &Params) -> Tr<Value> {
    check_keys(
        part,
        "ETBReplacement",
        &["DB", "ETB", "CounterType"],
        |_, _| false,
    )?;
    let names: Vec<&str> = part
        .get("CounterType")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .collect();
    if part.get("ETB") != Some("True") || names.len() < 2 {
        return unmapped("keyword:ETBReplacement-PutCounter");
    }
    let kinds = names
        .iter()
        .map(|name| super::amount::counter_kind(name))
        .collect::<Option<Vec<Value>>>();
    match kinds {
        Some(kinds) => Ok(json!({ "WithCounterOf": kinds })),
        None => unmapped("keyword:ETBReplacement-counter-kind"),
    }
}

/// What a `ChooseType` part of an entering choice makes the card remember: a creature type, a land type or a card type.
pub fn chosen_kind(part: &Params) -> Option<&'static str> {
    match part.get("Type")? {
        "Creature" => Some("CreatureType"),
        "Basic Land" | "Land" => Some("LandType"),
        "Card" => Some("CardType"),
        _ => None,
    }
}
