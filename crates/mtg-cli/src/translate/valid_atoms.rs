//! Properties of a valid-string that the DSL's selector atoms stand for: how an object was cast, where it sits in a
//! library, who blocks it and what happened to it this turn.

use serde_json::{Value, json};

use super::ctx::{CARD_TYPES, Ctx, Tr, unmapped};
use super::valid::{defined, objects, selector_any};

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
        "IsMonstrous" => return one(json!("Monstrous")),
        "IsRenowned" => return one(json!("Renowned")),
        "faceDown" => return one(json!("FaceDown")),
        "faceUp" => return one(json!({"Not": "FaceDown"})),
        "nonColorless" => return one(json!({"Not": "Colorless"})),
        // Only an attacker that no creature was declared to block; `blocked` is not its negation.
        "unblocked" => return one(json!("Unblocked")),
        "PairedWith" => return one(json!({"PairedWith": "This"})),
        "milledThisTurn" => return one(json!("MilledThisTurn")),
        "MonoColor" => return one(compare("Colors", "Eq", 1)),
        _ => {}
    }
    if let Some(found) = counted(prop) {
        return Some(vec![found]);
    }
    was_cast(prop.strip_prefix("wasCast")?)
}

/// Whether a reference names players only (a word that does not is a gap, or an object whose controller is wanted).
fn is_player(reference: &Value) -> bool {
    match reference {
        Value::String(word) => matches!(
            word.as_str(),
            "You" | "Opponents" | "EachPlayer" | "AttachedPlayer" | "Defending" | "ActivePlayer"
        ),
        Value::Object(map) => map.contains_key("ControllerOf") || map.contains_key("OwnerOf"),
        _ => false,
    }
}

/// The objects a word names: a valid-string (`Creature.YouCtrl`) as the cards that match it, else a `Defined$` word.
fn objects_of(ctx: &Ctx, word: &str) -> Tr<Value> {
    let word = word.trim();
    let head = word.split('.').next().unwrap_or(word);
    let base = CARD_TYPES.contains(&head)
        || matches!(head, "Permanent" | "Card")
        || ctx.world.subtypes.contains(head);
    if base {
        Ok(json!({"Each": selector_any(ctx, word)?}))
    } else {
        objects(defined(ctx, word)?)
    }
}

/// The player a `ControlledBy <word>` or `<word>Ctrl` names: the player a word holds, or the controller of the objects it
/// names (`Self`, `Enchanted`, `Valid ...`); `None` for a word that is neither.
fn controller_of(ctx: &Ctx, word: &str) -> Option<Value> {
    let word = word.trim();
    let found = defined(ctx, word).ok()?;
    if is_player(&found) {
        return Some(found);
    }
    let objects = word.starts_with("Valid ")
        || matches!(word, "Self" | "Card.Self" | "Enchanted" | "Equipped");
    objects.then(|| json!({ "ControllerOf": found }))
}

/// Properties that carry an argument or an object word: `AttachedTo <valid>`, `EnchantedBy <valid>`, `attacking <player>`,
/// `ControlledBy <word>`, `OwnedBy <word>`. `None` for any other property.
pub fn argument(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    let Some((head, arg)) = prop.split_once(' ') else {
        return Ok(None);
    };
    let one = |value: Value| Ok(Some(vec![value]));
    match head {
        // This attachment sits on one of the objects the word names.
        "AttachedTo" => one(json!({"AttachedTo": objects_of(ctx, arg)?})),
        // The object has an attachment that matches: the opposite direction of `AttachedTo`.
        "EnchantedBy" | "EquippedBy" => {
            let subtype = if head == "EnchantedBy" {
                "Aura"
            } else {
                "Equipment"
            };
            let mut attached = selector_any(ctx, arg)?;
            if !attached.contains(&json!({"Subtype": subtype})) {
                attached.push(json!({"Subtype": subtype}));
            }
            one(json!({ "HasAttached": attached }))
        }
        "attacking" => match defined(ctx, arg) {
            Ok(who) if is_player(&who) => one(json!({"AttackingPlayer": who})),
            _ => unmapped(format!("valid:attacking {arg}")),
        },
        "ControlledBy" | "OwnedBy" => {
            let Some(who) = controller_of(ctx, arg) else {
                return Ok(None);
            };
            let pred = if head == "ControlledBy" {
                "ControlledBy"
            } else {
                "OwnedBy"
            };
            one(json!({ pred: who }))
        }
        _ => Ok(None),
    }
}

/// `RememberedPlayerCtrl`, `EnchantedPlayerOwn`: the word names the player, the suffix says control or ownership.
pub fn suffixed(ctx: &Ctx, prop: &str) -> Option<Vec<Value>> {
    let (word, pred) = prop
        .strip_suffix("Ctrl")
        .map(|word| (word, "ControlledBy"))
        .or_else(|| prop.strip_suffix("Own").map(|word| (word, "OwnedBy")))?;
    let who = controller_of(ctx, word)?;
    Some(vec![json!({ pred: who })])
}

/// `numColorsEQ2`, `basePowerGE3`: a characteristic compared with a written number.
fn counted(prop: &str) -> Option<Value> {
    let (stat, rest) = [("numColors", "Colors"), ("basePower", "BasePower")]
        .iter()
        .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest)))?;
    let op = super::valid::cmp(rest.get(..2)?)?;
    let digits = rest.get(2..)?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(compare(stat, op, digits.parse().ok()?))
}

fn compare(stat: &str, op: &str, to: i64) -> Value {
    json!({"Compare": {"stat": stat, "op": op, "to": to}})
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
