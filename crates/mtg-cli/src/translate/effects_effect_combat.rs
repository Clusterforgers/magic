//! Combat statics of an `Effect` line whose subject is named through what the effect remembers: "creatures target
//! player controls can't block this turn", "other creatures can't attack this turn", "target creature with defender can
//! attack this turn as though it didn't have defender", "target creature attacks you this turn if able".

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::{objects, selector_any};
use crate::forge::script::Params;

/// The modes this module reads.
const MODES: [&str; 8] = [
    "MustAttack",
    "CantAttack",
    "CantBlock",
    "CantAttack,CantBlock",
    "CantBlock,CantAttack",
    "CantAttack,CantBlock,CantBeActivated",
    "CanAttackDefender",
    "CombatDamageToughness",
];

/// The effects of one static of an `Effect` about `who`, the objects or the player it remembers, for `duration`; `None`
/// for a static this module does not read.
pub fn statics(
    ctx: &mut Ctx,
    part: &Params,
    who: &Value,
    duration: &Value,
) -> Option<Tr<Vec<Value>>> {
    let mode = part.get("Mode").unwrap_or_default();
    if !MODES.contains(&mode) {
        return None;
    }
    let key = if mode == "MustAttack" {
        "ValidCreature"
    } else {
        "ValidCard"
    };
    // A line about the remembered objects themselves with no more than the key is the general reader's.
    let plain = is_remembered(part.get(key)) && mode != "MustAttack" && !mode.contains(',');
    if plain && matches!(mode, "CantAttack" | "CantBlock") {
        return None;
    }
    Some(read(ctx, part, mode, key, who, duration))
}

fn read(
    ctx: &mut Ctx,
    part: &Params,
    mode: &str,
    key: &str,
    who: &Value,
    duration: &Value,
) -> Tr<Vec<Value>> {
    let read: &[&str] = if mode == "MustAttack" {
        &["Mode", "ValidCreature", "MustAttack"]
    } else {
        &["Mode", "ValidCard"]
    };
    check_keys(part, "Effect.static", read, |_, _| false)?;
    let subject = subject(ctx, part.get(key), who)?;
    let rule = |rule: Value| json!({"Rule": {"rule": rule, "duration": duration}});
    let held = |mods: Value| json!({"Continuous": {"affected": subject, "mods": mods, "duration": duration}});
    Ok(match mode {
        "MustAttack" => match part.get("MustAttack") {
            None => vec![rule(json!({ "MustAttack": subject }))],
            // One creature bound to attack the effect's controller: the pair of the creature and that player.
            Some("You") if is_remembered(part.get(key)) => {
                vec![rule(json!({"MustAttackPlayers": {
                "who": subject, "players": "You"}}))]
            }
            Some(other) => return unmapped(format!("key:Effect.static.MustAttack={other}")),
        },
        "CantAttack" => vec![held(json!(["CantAttack"]))],
        "CantBlock" => vec![held(json!(["CantBlock"]))],
        "CantAttack,CantBlock" | "CantBlock,CantAttack" => {
            vec![held(json!(["CantAttack", "CantBlock"]))]
        }
        // "Can't attack or block, and its activated abilities can't be activated": mana abilities too (CR 605).
        "CantAttack,CantBlock,CantBeActivated" => vec![
            held(json!(["CantAttack", "CantBlock"])),
            rule(
                json!({"Cant": {"who": "EachPlayer", "action": {"ActivateKind": {
                "kind": "Every", "sources": sources(&subject)}}}}),
            ),
        ],
        "CanAttackDefender" => vec![rule(
            json!({"Combat": {"who": subject, "quirk": "AttackDespiteDefender"}}),
        )],
        "CombatDamageToughness" => vec![rule(
            json!({"Combat": {"who": subject, "quirk": "AssignsByToughness"}}),
        )],
        _ => return unmapped(format!("effect:Effect-static-{mode}")),
    })
}

/// Whether a valid-string names the remembered objects themselves.
fn is_remembered(valid: Option<&str>) -> bool {
    matches!(valid, Some("Card.IsRemembered" | "Creature.IsRemembered"))
}

/// The objects a key names: the remembered objects themselves, or the objects a valid-string picks with the remembered
/// player or objects read as `who` ("creatures target player controls", "other creatures").
fn subject(ctx: &mut Ctx, valid: Option<&str>, who: &Value) -> Tr<Value> {
    let Some(valid) = valid else {
        return unmapped("key:Effect.static.no-subject");
    };
    if is_remembered(Some(valid)) {
        return objects(who.clone());
    }
    if !valid.contains("Remembered") {
        return unmapped(format!(
            "valid:Effect.static-not-about-the-remembered={valid}"
        ));
    }
    let pinned = ctx.pinned.replace(who.clone());
    let remembered = ctx.remembered.take();
    let found = selector_any(ctx, valid);
    ctx.pinned = pinned;
    ctx.remembered = remembered;
    Ok(json!({ "Each": found? }))
}

/// The sources of a "can't be activated" rule about the subject.
fn sources(subject: &Value) -> Value {
    match subject.get("Each") {
        Some(selector) => selector.clone(),
        None => json!([{ "Is": subject }]),
    }
}
