//! The other creature of a block as a filter of a trigger (`ValidBlocked$` of `Blocks`, `ValidBlocker$` of
//! `AttackerBlocked`): "blocks a creature with flying" once for each such attacker, "becomes blocked by an artifact
//! creature" once for each such blocker (CR 509.3d), and "blocks or becomes blocked by one or more black creatures"
//! once, with the other creature checked as it triggers.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The line with its other-creature filter read: a per-creature filter becomes the line of
/// `AttackerBlockedByCreature` (one trigger for each pair), "one or more" is a condition checked as the ability
/// triggers, and a filter of every creature drops. Other lines pass unchanged.
pub fn g9a_block_filter(
    ctx: &Ctx,
    p: Params,
    intervening: Option<Value>,
) -> Tr<(Params, Option<Value>)> {
    let (key, blocks) = match p.get("Mode") {
        Some("Blocks") => ("ValidBlocked", true),
        Some("AttackerBlocked") => ("ValidBlocker", false),
        _ => return Ok((p, intervening)),
    };
    let (Some(other), Some(subject)) = (p.get(key), p.get("ValidCard")) else {
        return Ok((p, intervening));
    };
    let (other, subject) = (other.to_owned(), subject.to_owned());
    // Kept as before: a blocker that is the card or its attachment reads as `by`.
    if !blocks && super::triggers::attached_or_self(&other) {
        return Ok((p, intervening));
    }
    let gap = || unmapped(format!("key:Trigger.{}.{key}", mode_name(blocks)));
    let text = p.get("TriggerDescription").unwrap_or_default().to_owned();
    let single = if blocks {
        text.contains(" blocks a") && !text.contains("blocks or")
    } else {
        text.contains("becomes blocked by a")
    };
    // "Whenever ~ becomes blocked" written from the blocker's side: a creature blocks the card.
    if blocks && other == "Card.Self" && matches!(subject.as_str(), "Creature" | "Card") {
        return Ok((
            p.without(&[key, "ValidCard", "Mode"])
                .with("Mode", "AttackerBlocked")
                .with("ValidCard", "Card.Self"),
            intervening,
        ));
    }
    let Some(who) = one_object(&subject) else {
        return gap();
    };
    if single {
        // One trigger for each pair: the attacker filter and the blocker filter of `AttackerBlockedByCreature`.
        let (attacker, blocker) = if blocks {
            (other.as_str(), subject.as_str())
        } else {
            (subject.as_str(), other.as_str())
        };
        let line = p
            .without(&[key, "ValidCard", "Mode"])
            .with("Mode", "AttackerBlockedByCreature")
            .with("ValidCard", attacker)
            .with("ValidBlocker", blocker);
        return Ok((line, intervening));
    }
    if other == "Creature" {
        return Ok((p.without(&[key]), intervening));
    }
    if !text.contains("one or more") {
        return gap();
    }
    let mut found = selector_any(ctx, &other)?;
    found.push(json!({"AnyOf": [{"BlockedBy": who}, {"Blocks": who}]}));
    let some =
        json!({"AtTrigger": {"Compare": {"left": {"Count": found}, "op": "Ge", "right": 1}}});
    let both = match intervening {
        None => some,
        Some(before) => json!({"All": [before, some]}),
    };
    Ok((p.without(&[key]), Some(both)))
}

/// The Forge mode the gap names.
fn mode_name(blocks: bool) -> &'static str {
    if blocks { "Blocks" } else { "BecomesBlocked" }
}

/// The one object a subject word is: the card or the creature it is attached to.
fn one_object(word: &str) -> Option<&'static str> {
    match word {
        "Card.Self" => Some("This"),
        "Card.EquippedBy" | "Card.AttachedBy" | "Creature.EquippedBy" | "Creature.EnchantedBy" => {
            Some("Attached")
        }
        _ => None,
    }
}

/// "A spell with the same name as a card in their graveyard" of a spell another player casts: Forge's `YourGraveyard`
/// is the caster's graveyard there, which is the graveyard of the spell's controller (`Ref::Candidate` is the spell
/// tested), not that of the trigger's controller.
pub fn g9a_caster_graveyard(pattern: &mut Value) {
    let yours = json!({"Each": [{"In": "Graveyard"}, {"OwnedBy": "You"}]});
    let theirs = json!({"Each": [{"In": "Graveyard"}, {"OwnedBy": {"ControllerOf": "Candidate"}}]});
    walk(pattern, &yours, &theirs);
}

/// Replaces each `SharesName` pool equal to `yours` with `theirs`.
fn walk(value: &mut Value, yours: &Value, theirs: &Value) {
    match value {
        Value::Object(map) => {
            if let Some(pool) = map.get_mut("SharesName")
                && pool == yours
            {
                *pool = theirs.clone();
            }
            map.values_mut()
                .for_each(|inner| walk(inner, yours, theirs));
        }
        Value::Array(list) => list.iter_mut().for_each(|inner| walk(inner, yours, theirs)),
        _ => {}
    }
}
