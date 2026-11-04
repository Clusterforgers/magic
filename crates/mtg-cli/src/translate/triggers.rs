//! Triggers: a `T:` line as the DSL's `Triggered` ability, its event pattern and the effects of its `Execute$` part.

use serde_json::{Value, json};

use super::condition::{condition, strip};
use super::ctx::{Ctx, Tr, unmapped};
use super::effects::chain_at;
use super::valid::selector_any;
use crate::forge::script::Params;

/// The keys every trigger line may carry besides its mode's own.
const COMMON: [&str; 6] = [
    "ActivationLimit",
    "Mode",
    "Execute",
    "TriggerZones",
    "OptionalDecider",
    "NoResolvingCheck",
];

/// A trigger line as a `Triggered` ability.
pub fn triggered(ctx: &mut Ctx, body: &str) -> Tr<Value> {
    ctx.reset();
    let all = Params::parse(body);
    let intervening = condition(ctx, &all)?;
    // The engine checks the condition again as the ability resolves (CR 603.4); Forge's line says it does not.
    if all.get("NoResolvingCheck") == Some("True") && intervening.is_some() {
        return unmapped("key:Trigger.NoResolvingCheck");
    }
    let p = strip(&all);
    let when = pattern(ctx, &p)?;
    let itself = is_about_itself(&when);
    ctx.trigger = Some(super::triggered::Shape {
        mode: p.get("Mode").unwrap_or_default().to_owned(),
        itself,
        by: when
            .get("BecomesBlocked")
            .is_some_and(|node| node.get("by").is_some()),
    });
    let from = match p.get("TriggerZones") {
        None | Some("Battlefield") => None,
        Some(zone @ ("Graveyard" | "Hand" | "Exile" | "Library")) => Some(zone),
        Some(other) => return unmapped(format!("key:Trigger.TriggerZones={other}")),
    };
    let Some(execute) = p.get("Execute") else {
        return unmapped("trigger:no-Execute");
    };
    let part = ctx.sub(execute)?;
    let mut effects = chain_at(ctx, &part, false)?;
    // An event of a player (a life gain, a step, a spell cast) has no object to change.
    let about_a_player = when
        .as_object()
        .and_then(|map| map.values().next())
        .is_some_and(|inner| inner.get("what").is_none());
    if about_a_player
        && json!(effects)
            .to_string()
            .contains("\"affected\":\"Triggering\"")
    {
        return unmapped("trigger:object-effect-on-a-player-event");
    }
    // Paying a cost is a choice already: no "you may" beside "you may pay".
    let pays = matches!(effects.as_slice(), [only] if only.get("Pay").is_some());
    match p.get("OptionalDecider") {
        None => {}
        Some("You" | "TriggeredCardController") if pays => {}
        Some("You") => effects = vec![json!({"May": {"effects": effects}})],
        Some("TriggeredCardController") if itself => {
            effects = vec![json!({"May": {"effects": effects}})];
        }
        Some(other) => return unmapped(format!("key:Trigger.OptionalDecider={other}")),
    }
    let mut ability = json!({"when": when, "effects": effects});
    if let Some(zone) = from {
        ability["from"] = json!(zone);
    }
    if let Some(condition) = intervening {
        ability["if"] = condition;
    }
    match p.get("ActivationLimit") {
        None => {}
        Some("1") => ability["limit"] = json!("OncePerTurn"),
        Some(other) => return unmapped(format!("key:Trigger.ActivationLimit={other}")),
    }
    if !ctx.targets.is_empty() {
        ability["targets"] = Value::Array(ctx.targets.clone());
    }
    Ok(json!({ "Triggered": ability }))
}

/// Whether a pattern is about the card itself ("when this creature dies"), so that "that creature" is the card and its
/// controller is "you".
fn is_about_itself(when: &Value) -> bool {
    when.as_object()
        .and_then(|map| map.values().next())
        .and_then(|inner| inner.get("what"))
        .is_some_and(|what| *what == json!(["This"]))
}

/// Checks the keys of a trigger line against those a mode reads.
pub(super) fn keys(p: &Params, mode: &str, read: &[&str]) -> Tr<()> {
    super::ctx::check_keys(p, &format!("Trigger.{mode}"), read, |key, _| {
        COMMON.contains(&key)
    })
}

fn pattern(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let mode = p.get("Mode").unwrap_or_default();
    if let Some(found) = super::triggers_more::pattern(ctx, p, mode) {
        return found;
    }
    if let Some(found) = super::triggers_did::pattern(ctx, p, mode) {
        return found;
    }
    match mode {
        "ChangesZone" => changes_zone(ctx, p),
        "Phase" => phase(p),
        "Attacks" => attacks(ctx, p),
        "AttackersDeclared" => attackers_declared(ctx, p),
        "Discarded" => card_event(ctx, p, "Discarded"),
        "Cycled" => card_event(ctx, p, "Cycled"),
        "TurnFaceUp" => what_trigger(ctx, p, "TurnedFaceUp"),
        "SpellCast" => spell_cast(ctx, p),
        "DamageDone" => super::triggers_more::damage(ctx, p, "DamageDone"),
        "Drawn" => drawn(p),
        "LifeGained" => life_gained(p),
        "Sacrificed" => sacrificed(ctx, p),
        "Taps" => what_trigger(ctx, p, "Tapped"),
        "Untaps" => what_trigger(ctx, p, "Untapped"),
        "Blocks" => what_trigger(ctx, p, "Blocks"),
        "AttackerBlocked" => what_trigger(ctx, p, "BecomesBlocked"),
        other => unmapped(format!("trigger:{other}")),
    }
}

/// A zone word of a trigger (`Any` is no zone).
fn zone(word: Option<&str>, key: &str) -> Tr<Option<&'static str>> {
    match word {
        None | Some("Any") => Ok(None),
        Some("Battlefield") => Ok(Some("Battlefield")),
        Some("Graveyard") => Ok(Some("Graveyard")),
        Some("Hand") => Ok(Some("Hand")),
        Some("Library") => Ok(Some("Library")),
        Some("Exile") => Ok(Some("Exile")),
        Some("Stack") => Ok(Some("Stack")),
        Some(other) => unmapped(format!("zone:Trigger.{key}={other}")),
    }
}

fn changes_zone(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "ChangesZone", &["ValidCard", "Origin", "Destination"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let mut pattern = json!({ "what": what });
    if let Some(from) = zone(p.get("Origin"), "Origin")? {
        pattern["from"] = json!(from);
    }
    if let Some(to) = zone(p.get("Destination"), "Destination")? {
        pattern["to"] = json!(to);
    }
    Ok(json!({ "ZoneChange": pattern }))
}

/// The step a Forge `Phase$` word names.
pub fn step(word: &str) -> Tr<&'static str> {
    Ok(match word {
        "Upkeep" => "Upkeep",
        "Draw" => "Draw",
        "Main1" => "Main1",
        "Main2" => "Main2",
        "BeginCombat" => "BeginCombat",
        "EndCombat" => "EndCombat",
        "End of Turn" | "End Of Turn" => "End",
        "Untap" => "Untap",
        other => return unmapped(format!("key:Trigger.Phase={other}")),
    })
}

fn phase(p: &Params) -> Tr<Value> {
    keys(p, "Phase", &["Phase", "ValidPlayer"])?;
    let step = step(p.get("Phase").unwrap_or_default())?;
    let whose = match p.get("ValidPlayer") {
        None | Some("Player") => "EachPlayer",
        Some("You") => "You",
        Some("Opponent" | "Player.Opponent") => "Opponents",
        Some(other) => return unmapped(format!("key:Trigger.Phase.ValidPlayer={other}")),
    };
    Ok(json!({"StepBegins": {"step": step, "whose": whose}}))
}

fn attacks(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Attacks", &["ValidCard", "Alone"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Creature"))?;
    let variant = if p.get("Alone") == Some("True") {
        "AttacksAlone"
    } else {
        "Attacks"
    };
    Ok(json!({ variant: {"what": what} }))
}

/// The cards a player-related trigger is about and the player: the controller or owner in the valid-string is the player.
fn card_and_player(ctx: &Ctx, valid: &str) -> Tr<(Vec<Value>, Option<&'static str>)> {
    let mut what = selector_any(ctx, valid)?;
    let mut by = None;
    for (who, name) in [
        (json!({"ControlledBy": "You"}), "You"),
        (json!({"OwnedBy": "You"}), "You"),
        (json!({"OwnedBy": "Opponents"}), "Opponents"),
        (json!({"ControlledBy": "Opponents"}), "Opponents"),
    ] {
        if let Some(at) = what.iter().position(|pred| *pred == who) {
            what.remove(at);
            by = Some(name);
        }
    }
    Ok((what, by))
}

/// `Discarded` and `Cycled`: a card is discarded or cycled, by a player or as itself.
fn card_event(ctx: &Ctx, p: &Params, variant: &str) -> Tr<Value> {
    keys(p, variant, &["ValidCard"])?;
    let (what, by) = card_and_player(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let mut pattern = json!({});
    if !what.is_empty() {
        pattern["what"] = json!(what);
    }
    if let Some(by) = by {
        pattern["by"] = json!(by);
    }
    Ok(json!({ variant: pattern }))
}

/// "Whenever you attack" and "whenever you attack with two or more creatures".
fn attackers_declared(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "AttackersDeclared",
        &["AttackingPlayer", "ValidAttackers", "ValidAttackersAmount"],
    )?;
    if p.get("AttackingPlayer") != Some("You") {
        return unmapped("key:Trigger.AttackersDeclared.AttackingPlayer");
    }
    let mut what = selector_any(ctx, p.get("ValidAttackers").unwrap_or("Creature"))?;
    if !what.contains(&json!({"ControlledBy": "You"})) {
        what.push(json!({"ControlledBy": "You"}));
    }
    let mut pattern = json!({ "what": what });
    match p.get("ValidAttackersAmount") {
        None => {}
        Some(amount) => match amount
            .strip_prefix("GE")
            .and_then(|n| n.parse::<u32>().ok())
        {
            Some(least) if least >= 2 => pattern["at_least"] = json!(least),
            _ => return unmapped("key:Trigger.AttackersDeclared.ValidAttackersAmount"),
        },
    }
    Ok(json!({ "GroupAttacks": pattern }))
}

pub(super) fn player_ref(word: Option<&str>) -> Tr<&'static str> {
    match word {
        None | Some("Player") => Ok("EachPlayer"),
        Some("You") => Ok("You"),
        Some("Opponent" | "Player.Opponent") => Ok("Opponents"),
        Some(other) => unmapped(format!("player:{other}")),
    }
}

fn spell_cast(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(
        p,
        "SpellCast",
        &[
            "ValidCard",
            "ValidActivatingPlayer",
            "ActivatorThisTurnCast",
        ],
    )?;
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut pattern = json!({ "by": by });
    if let Some(valid) = p.get("ValidCard") {
        pattern["what"] = json!(selector_any(ctx, valid)?);
    }
    super::triggers_more::nth_spell(p, json!({ "SpellCast": pattern }))
}

/// A permanent is sacrificed, by a player.
fn sacrificed(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Sacrificed", &["ValidCard", "ValidPlayer"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let by = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"Sacrificed": {"what": what, "by": by}}))
}

fn drawn(p: &Params) -> Tr<Value> {
    keys(p, "Drawn", &["ValidCard"])?;
    let who = match p.get("ValidCard") {
        Some("Card.YouOwn" | "Card.YouCtrl") => "You",
        Some("Card.OppOwn") => "Opponents",
        None | Some("Card") => "EachPlayer",
        Some(other) => return unmapped(format!("key:Trigger.Drawn.ValidCard={other}")),
    };
    Ok(json!({"Drew": {"who": who}}))
}

fn life_gained(p: &Params) -> Tr<Value> {
    keys(p, "LifeGained", &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"LifeGained": {"who": who}}))
}

/// A trigger on a permanent: sacrificed, tapped, untapped, blocks, becomes blocked.
fn what_trigger(ctx: &Ctx, p: &Params, variant: &str) -> Tr<Value> {
    keys(p, variant, &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    Ok(json!({ variant: {"what": what} }))
}
