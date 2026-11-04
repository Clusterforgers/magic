//! Properties of a valid-string that read the translation's context or a game fact: who is kicked, active or defending,
//! the zone a card sits in, the player a target names and the outlaw creature types.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::memory::{self, Slot};
use super::valid::{defined, objects};

/// The zones a property such as `inZoneHand` names.
const ZONES: [&str; 7] = [
    "Battlefield",
    "Hand",
    "Library",
    "Graveyard",
    "Stack",
    "Exile",
    "Command",
];

/// The creature types that make an outlaw.
const OUTLAW_TYPES: [&str; 5] = ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"];

/// The predicates of a property that is one of these, `None` for any other property.
pub fn property(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    let one = |value: Value| Ok(Some(vec![value]));
    match prop {
        "kicked" => return one(json!("Kicked")),
        "ActivePlayerCtrl" => return one(json!({"ControlledBy": "ActivePlayer"})),
        "ActivePlayerOwn" => return one(json!({"OwnedBy": "ActivePlayer"})),
        "attackedThisTurn" => return one(json!("AttackedThisTurn")),
        "DefenderCtrl" => return one(json!({"ControlledBy": "Defending"})),
        "TargetedPlayerCtrl" => return one(json!({"ControlledBy": ctx.player_target()?})),
        "TargetedPlayerOwn" => return one(json!({"OwnedBy": ctx.player_target()?})),
        // Without an argument the property compares with the card the line belongs to.
        "sharesCreatureTypeWith" => return one(json!({"SharesSubtype": "This"})),
        "Outlaw" => return outlaw(ctx).map(Some),
        "IsRemembered" => return memory::is_in(ctx, Slot::Remembered).map(Some),
        "IsImprinted" => return memory::is_in(ctx, Slot::Imprinted).map(Some),
        "NamedCard" => return one(memory::entered_with(ctx, prop)?),
        "IsNotChosenType" => {
            return one(json!({"Not": memory::entered_with(ctx, "ChosenType")?}));
        }
        _ => {}
    }
    if let Some(zone) = prop.strip_prefix("inZone")
        && ZONES.contains(&zone)
    {
        return one(json!({"In": zone}));
    }
    if let Some(found) = shares(ctx, prop)? {
        return Ok(Some(vec![found]));
    }
    if prop == "NotDefinedTargeted" {
        return one(json!({"Not": {"Is": objects(ctx.last_target()?)?}}));
    }
    if let Some(who) = prop.strip_prefix("ControlledBy ") {
        return one(json!({"ControlledBy": player_word(ctx, who)?}));
    }
    if let Some(who) = prop.strip_prefix("OwnedBy ") {
        return one(json!({"OwnedBy": player_word(ctx, who)?}));
    }
    Ok(None)
}

/// `sharesNameWith Targeted`: the object has the name, card type, color or creature type of the objects the word names.
fn shares(ctx: &Ctx, prop: &str) -> Tr<Option<Value>> {
    for (head, pred) in [
        ("sharesNameWith ", "SharesName"),
        ("sharesCardTypeWith ", "SharesType"),
        ("sharesColorWith ", "SharesColor"),
        ("sharesCreatureTypeWith ", "SharesSubtype"),
    ] {
        if let Some(word) = prop.strip_prefix(head) {
            return Ok(Some(json!({ pred: objects(defined(ctx, word)?)? })));
        }
    }
    Ok(None)
}

/// An outlaw: a creature of one of the outlaw types.
fn outlaw(ctx: &Ctx) -> Tr<Vec<Value>> {
    if !OUTLAW_TYPES
        .iter()
        .all(|name| ctx.world.subtypes.contains(*name))
    {
        return unmapped("valid:Outlaw");
    }
    let any: Vec<Value> = OUTLAW_TYPES
        .iter()
        .map(|name| json!({"Subtype": name}))
        .collect();
    Ok(vec![json!({"AnyOf": any})])
}

/// The player a `ControlledBy <word>` property names, for the words that hold a player.
fn player_word(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word.trim() {
        "You" => Ok(json!("You")),
        "DefendingPlayer" | "TriggeredDefendingPlayer" => Ok(json!("Defending")),
        "TargetedPlayer" => ctx.player_target(),
        other => unmapped(format!("valid:ControlledBy {other}")),
    }
}

/// The reference of a `Defined$` word the table does not know; every word is still a gap.
pub fn defined_word(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "Imprinted" | "Player.IsImprinted" => memory::imprinted(ctx),
        _ => unmapped(format!("defined:{word}")),
    }
}
