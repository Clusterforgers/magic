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
        "bargained" | "Teamwork" => return one(json!("AdditionalCostPaid")),
        "ActivePlayerCtrl" => return one(json!({"ControlledBy": "ActivePlayer"})),
        "ActivePlayerOwn" => return one(json!({"OwnedBy": "ActivePlayer"})),
        "attackedThisTurn" => return one(json!("AttackedThisTurn")),
        "DefenderCtrl" => return one(json!({"ControlledBy": "Defending"})),
        "TargetedPlayerCtrl" => return one(json!({"ControlledBy": ctx.player_target()?})),
        "TargetedPlayerOwn" => return one(json!({"OwnedBy": ctx.player_target()?})),
        // Without an argument the property compares with the card the line belongs to.
        "sharesCreatureTypeWith" => return one(json!({"SharesSubtype": "This"})),
        "Outlaw" => return outlaw(ctx).map(Some),
        // The player the source chose; outside the battlefield control is ownership.
        "ChosenCtrl" | "ChosenOwn" => {
            let who = super::valid::chosen_player(ctx, &format!("valid:{prop}-without-a-choice"))?;
            let key = if prop == "ChosenCtrl" {
                "ControlledBy"
            } else {
                "OwnedBy"
            };
            return one(json!({ key: who }));
        }
        "IsRemembered" => return memory::is_in(ctx, Slot::Remembered).map(Some),
        "IsTriggerRemembered" => {
            return match &ctx.pinned {
                Some(who) => one(json!({"Is": who})),
                None => unmapped("valid:IsTriggerRemembered"),
            };
        }
        "IsImprinted" => return memory::is_in(ctx, Slot::Imprinted).map(Some),
        // A card named by an earlier step of the ability, else the name the source chose as it entered.
        "NamedCard" => {
            return match (&ctx.chosen, ctx.chosen_kind) {
                (Some(name), None) => one(json!({"ChosenAs": name})),
                _ => one(memory::entered_with(ctx, prop)?),
            };
        }
        "IsNotChosenType" => {
            let chosen = match &ctx.chosen {
                Some(name) => json!({"ChosenAs": name}),
                None => memory::entered_with(ctx, "ChosenType")?,
            };
            return one(json!({"Not": chosen}));
        }
        _ => {}
    }
    if let Some(zone) = prop
        .strip_prefix("inZone")
        .or_else(|| prop.strip_prefix("inRealZone"))
        && ZONES.contains(&zone)
    {
        return one(json!({"In": zone}));
    }
    if let Some(found) = shares(ctx, prop)? {
        return Ok(Some(vec![found]));
    }
    if let Some(at) = prop.strip_prefix("NotTarget")
        && let Ok(at) = at.parse::<usize>()
        && at < ctx.targets.len()
    {
        return one(json!({"Not": {"Is": {"Target": at}}}));
    }
    if prop == "NotDefinedTargeted" {
        return one(json!({"Not": {"Is": objects(ctx.last_target()?)?}}));
    }
    // "That player or that planeswalker's controller": a player target is itself, an object target is controlled by one.
    if prop == "ControlledBy TargetedOrController" {
        let target = ctx.last_target()?;
        return one(json!({"AnyOf": [
            {"ControlledBy": target},
            {"ControlledBy": {"ControllerOf": target}},
        ]}));
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
    if let Some(word) = prop.strip_prefix("doesNotShareNameWith ") {
        return Ok(Some(json!({"Not": {"SharesName": name_pool(ctx, word)?}})));
    }
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

/// The objects whose names `doesNotShareNameWith` compares with: your graveyard, or the objects a valid-string or a
/// `Defined$` word names. "Another" permanent (`OtherYourBattlefield`) is another than the object tested, which a
/// selector cannot say, so it stays a gap.
fn name_pool(ctx: &Ctx, word: &str) -> Tr<Value> {
    let word = word.trim();
    if word == "YourGraveyard" {
        return Ok(json!({"Each": [{"In": "Graveyard"}, {"OwnedBy": "You"}]}));
    }
    if word.contains('.') {
        return objects(defined(ctx, &format!("Valid {word}"))?);
    }
    if word.starts_with("Other") {
        return unmapped(format!("valid:doesNotShareNameWith {word}"));
    }
    objects(defined(ctx, word)?)
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
        // The player a damage trigger names (`ValidTarget$ Player`): the one the creatures are read for.
        "CardController" => {
            super::triggered::resolve(ctx.trigger.as_ref(), "TriggeredCardController")
        }
        other if super::triggered::is_word(other) => {
            super::triggered::resolve(ctx.trigger.as_ref(), other)
        }
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
