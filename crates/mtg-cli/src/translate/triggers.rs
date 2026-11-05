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
    // The engine checks the condition again as the ability resolves (CR 603.4) unless the line says it does not.
    let intervening = match (all.get("NoResolvingCheck"), intervening) {
        (Some("True"), Some(condition)) => Some(json!({ "AtTrigger": condition })),
        (_, other) => other,
    };
    let (p, intervening) = during(strip(&all), intervening)?;
    let (p, intervening) = event_amount(p, intervening)?;
    let (p, intervening) = defender_poisoned(p, intervening);
    let (when, intervening) = if p.get("Mode") == Some("Always") {
        state_when(ctx, &p, &all)?
    } else {
        (pattern(ctx, &p)?, intervening)
    };
    let itself = is_about_itself(&when);
    ctx.trigger = Some(super::triggered::Shape {
        mode: p.get("Mode").unwrap_or_default().to_owned(),
        itself,
        by: when
            .get("BecomesBlocked")
            .is_some_and(|node| node.get("by").is_some()),
        to_player: p.get("ValidTarget").is_some_and(|word| {
            word == "Player" || word.starts_with("Player.") || word == "Opponent"
        }),
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
    if let Some(found) = &intervening
        && when.get("SpellCast").is_some()
        && counts_the_cast_exactly(found)
    {
        // The count grows as more spells are cast, so a recheck at resolution would pass the first spell when a second
        // is cast in response; the engine has no first-of-the-turn event across all players.
        return unmapped("trigger:SpellCast-exact-count-recheck");
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
    let ability = json!({ "Triggered": ability });
    // A triggered mana ability (CR 605.1b) only works in the shape the engine folds into the tap.
    if p.get("Static") == Some("True") {
        let folds = serde_json::from_value::<mtg_cards::ability::Ability>(ability.clone())
            .is_ok_and(|found| mtg_cards::mana_trigger::mana_trigger(&found).is_some());
        if !folds {
            return unmapped("key:Trigger.TapsForMana.Static");
        }
    }
    Ok(ability)
}

/// The step a card enters in (`Phase$ Declare Attackers` of a zone change) as a condition of the event, checked as the ability
/// triggers; the line without the key and the condition joined with the intervening one.
fn during(p: Params, intervening: Option<Value>) -> Tr<(Params, Option<Value>)> {
    let Some(word) = p
        .get("Phase")
        .filter(|_| p.get("Mode") == Some("ChangesZone"))
    else {
        return Ok((p, intervening));
    };
    let state = match word {
        "Declare Attackers" => json!({"Step": "DeclareAttackers"}),
        "BeginCombat->EndCombat" => json!("InCombat"),
        "Main1,Main2" => json!({"Any": [{"Step": "Main1"}, {"Step": "Main2"}]}),
        _ => return unmapped(format!("key:Trigger.ChangesZone.Phase={word}")),
    };
    let during = json!({ "AtTrigger": state });
    let both = match intervening {
        None => during,
        Some(other) => json!({"All": [other, during]}),
    };
    Ok((p.without(&["Phase"]), Some(both)))
}

/// The size an event must have (`DamageAmount$ GE5`, `CounterAmount$ EQ3`, `LifeAmount$ EQ1`) as a condition of the event,
/// checked as the ability triggers: the amount of the event compared with a number. Other modes keep the key and name it.
fn event_amount(p: Params, intervening: Option<Value>) -> Tr<(Params, Option<Value>)> {
    let key = match p.get("Mode") {
        Some("DamageDone" | "DamageDoneOnce") => "DamageAmount",
        Some("CounterAdded") => "CounterAmount",
        Some("LifeGained" | "LifeLost") => "LifeAmount",
        _ => return Ok((p, intervening)),
    };
    let Some(word) = p.get(key) else {
        return Ok((p, intervening));
    };
    let (op, number) = word.split_at_checked(2).unwrap_or_default();
    let op = match op {
        "EQ" => "Eq",
        "NE" => "Ne",
        "LT" => "Lt",
        "LE" => "Le",
        "GT" => "Gt",
        "GE" => "Ge",
        _ => return unmapped(format!("key:Trigger.{key}={word}")),
    };
    let Ok(number) = number.parse::<u32>() else {
        return unmapped(format!("key:Trigger.{key}={word}"));
    };
    // The amount Forge compares for a counter put is the total on the object, not the number this event added.
    let left = if key == "CounterAmount" {
        let on_itself = p.get("ValidCard") == Some("Card.Self");
        let kind = p.get("CounterType").and_then(super::amount::counter_kind);
        match (on_itself, kind) {
            (true, Some(kind)) => json!({"Counters": {"on": "This", "counter": kind}}),
            _ => return unmapped("key:Trigger.CounterAmount.Total"),
        }
    } else {
        json!("Triggering")
    };
    let size = json!({"AtTrigger": {"Compare": {"left": left, "op": op, "right": number}}});
    let both = match intervening {
        None => size,
        Some(other) => json!({"All": [other, size]}),
    };
    Ok((p.without(&[key]), Some(both)))
}

/// `DefendingPlayerPoisoned$ True` ("if defending player is poisoned") of an attack: the player attacked has a poison counter.
fn defender_poisoned(p: Params, intervening: Option<Value>) -> (Params, Option<Value>) {
    if p.get("Mode") != Some("Attacks") || p.get("DefendingPlayerPoisoned") != Some("True") {
        return (p, intervening);
    }
    let poisoned = json!({"Compare": {"left": {"Counters": {"on": "Defending", "counter": super::amount::counter_kind("POISON")}}, "op": "Ge", "right": 1}});
    let both = match intervening {
        None => poisoned,
        Some(other) => json!({"All": [other, poisoned]}),
    };
    (p.without(&["DefendingPlayerPoisoned"]), Some(both))
}

/// "When you control no Islands": a state trigger holds the line's condition as its pattern and has no intervening one.
fn state_when(ctx: &Ctx, p: &Params, all: &Params) -> Tr<(Value, Option<Value>)> {
    keys(p, "Always", &[])?;
    let Some(state) = condition(ctx, all)? else {
        return unmapped("trigger:Always-without-condition");
    };
    Ok((json!({"State": {"condition": state}}), None))
}

/// Whether a pattern is about the card itself ("when this creature dies"), so that "that creature" is the card and its
/// controller is "you".
pub(super) fn is_about_itself(when: &Value) -> bool {
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

/// The event pattern of a `DelayedTrigger` line and whether it waits only for this turn: the line reads its `Mode$` like a
/// trigger line. "Next turn" is a step apart from "this turn" and stays a gap.
pub fn delayed_when(ctx: &Ctx, p: &Params) -> Tr<(Value, bool)> {
    if p.get("NextTurn").is_some() {
        return unmapped("key:DelayedTrigger.NextTurn");
    }
    let this_turn = match p.get("ThisTurn") {
        None => false,
        Some("True") => true,
        Some(other) => return unmapped(format!("key:DelayedTrigger.ThisTurn={other}")),
    };
    let line = p.without(&["ThisTurn", "RememberObjects"]);
    // Valid words of a delayed trigger are the same words a trigger line reads.
    let when = pattern(ctx, &line).map_err(|gap| match gap.0.strip_prefix("key:Trigger.") {
        Some(rest) => super::ctx::Unmapped(format!("key:DelayedTrigger.{rest}")),
        None => gap,
    })?;
    Ok((when, this_turn))
}

pub(super) fn pattern(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let mode = p.get("Mode").unwrap_or_default();
    if let Some(found) = super::triggers_more::pattern(ctx, p, mode) {
        return found;
    }
    if let Some(found) = super::triggers_did::pattern(ctx, p, mode) {
        return found;
    }
    if let Some(found) = super::triggers_batch::pattern(ctx, p, mode) {
        return found;
    }
    if let Some(found) = super::header::trigger_words(ctx, p, mode) {
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
        "AttackerBlocked" => attacker_blocked(ctx, p),
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
    keys(
        p,
        "ChangesZone",
        &["ValidCard", "Origin", "Destination", "TriggerController"],
    )?;
    // The controller of a trigger of a permanent leaving the battlefield is its controller as it left (CR 603.3a).
    match (p.get("TriggerController"), p.get("Origin")) {
        (None, _) | (Some("TriggeredCardController"), Some("Battlefield")) => {}
        _ => return unmapped("key:Trigger.ChangesZone.TriggerController"),
    }
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

/// Whether the printed trigger says "the end step", not "your end step".
fn prints_every_end_step(p: &Params) -> bool {
    p.get("TriggerDescription")
        .is_some_and(|text| text.to_lowercase().contains("beginning of the end step"))
}

fn phase(p: &Params) -> Tr<Value> {
    keys(p, "Phase", &["Phase", "ValidPlayer", "PhaseCount"])?;
    // `Phase$ Main` with a count names the first or the second (postcombat) main phase.
    let step = match (p.get("Phase"), p.get("PhaseCount")) {
        (Some("Main"), Some("1")) => "Main1",
        (Some("Main"), Some("2")) => "Main2",
        (Some("Main"), _) | (_, Some(_)) => return unmapped("key:Trigger.Phase.PhaseCount"),
        (word, None) => step(word.unwrap_or_default())?,
    };
    let whose = match p.get("ValidPlayer") {
        // Forge scripts "the end step" (every player's) with `ValidPlayer$ You`; the print has no "your".
        Some("You") if prints_every_end_step(p) => json!("EachPlayer"),
        word => match player_ref(word) {
            Ok(found) => found,
            Err(_) => {
                return unmapped(format!(
                    "key:Trigger.Phase.ValidPlayer={}",
                    word.unwrap_or_default()
                ));
            }
        },
    };
    Ok(json!({"StepBegins": {"step": step, "whose": whose}}))
}

fn attacks(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Attacks", &["ValidCard", "Alone", "Attacked"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Creature"))?;
    if let Some(word) = p.get("Attacked") {
        if p.get("Alone") == Some("True") {
            return unmapped("key:Trigger.Attacks.Attacked-with-Alone");
        }
        return attacks_at(word, &what);
    }
    let variant = if p.get("Alone") == Some("True") {
        "AttacksAlone"
    } else {
        "Attacks"
    };
    Ok(json!({ variant: {"what": what} }))
}

/// An attack on a player (`Attacked$ You`), or on a player or a planeswalker that player controls
/// (`Attacked$ You,Planeswalker.YouCtrl`): the two stay apart, and any other attacked thing is a gap.
fn attacks_at(word: &str, what: &[Value]) -> Tr<Value> {
    let gap = || unmapped(format!("key:Trigger.Attacks.Attacked={word}"));
    let mut parts = word.split(',');
    let Some(player) = parts.next() else {
        return gap();
    };
    let Ok(at) = player_ref(Some(player)) else {
        return gap();
    };
    let variant = match (parts.next(), parts.next(), &at) {
        (None, ..) => "AttacksPlayer",
        (Some("Planeswalker.YouCtrl"), None, Value::String(who)) if who == "You" => {
            "AttacksPlayerOrPlaneswalker"
        }
        (Some("Planeswalker.OppCtrl"), None, Value::String(who)) if who == "Opponents" => {
            "AttacksPlayerOrPlaneswalker"
        }
        (Some("Planeswalker.EnchantedPlayerCtrl"), None, Value::String(who))
            if who == "AttachedPlayer" =>
        {
            "AttacksPlayerOrPlaneswalker"
        }
        _ => return gap(),
    };
    Ok(json!({ variant: {"what": what, "at": at} }))
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
    // One player is attacked once in a declaration, so "attacks you" is the attackers at that one player.
    if matches!(p.get("AttackedTarget"), Some("You" | "Player.EnchantedBy")) {
        return super::triggers_r4::attackers_at_one_target(ctx, p, "AttackersDeclared");
    }
    keys(
        p,
        "AttackersDeclared",
        &["AttackingPlayer", "ValidAttackers", "ValidAttackersAmount"],
    )?;
    let word = p.get("AttackingPlayer");
    let Ok(by) = player_ref(word) else {
        return unmapped(format!(
            "key:Trigger.AttackersDeclared.AttackingPlayer={}",
            word.unwrap_or_default()
        ));
    };
    let mut what = selector_any(ctx, p.get("ValidAttackers").unwrap_or("Creature"))?;
    // The attackers are the attacking player's own creatures.
    let own = json!({ "ControlledBy": by });
    if by != "EachPlayer" && !what.contains(&own) {
        what.push(own);
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

/// The player a trigger's player word names: the controller, the opponents, anyone, the player an Aura enchants or that
/// permanent's controller.
pub(super) fn player_ref(word: Option<&str>) -> Tr<Value> {
    match word {
        None | Some("Player") => Ok(json!("EachPlayer")),
        Some("You") => Ok(json!("You")),
        // Teams do not exist at our tables: the other players are the opponents.
        Some("Opponent" | "Player.Opponent" | "Player.Other") => Ok(json!("Opponents")),
        Some("Player.EnchantedBy") => Ok(json!("AttachedPlayer")),
        Some("Player.EnchantedController") => Ok(json!({"ControllerOf": "Attached"})),
        Some("Player.Chosen") => Ok(json!("Chosen")),
        Some("Player.TriggeredDefendingPlayer" | "TriggeredDefendingPlayer") => {
            Ok(json!("Defending"))
        }
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
            "TargetsValid",
            "ValidSA",
            "ValidSAonCard",
            "IsSingleTarget",
        ],
    )?;
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut pattern = json!({ "by": by });
    if let Some(valid) = p.get("ValidCard") {
        pattern["what"] = json!(selector_any(ctx, valid)?);
    }
    // The spell ability's own words ("an instant or sorcery spell with a single target") restrict the same spell.
    for key in ["ValidSA", "ValidSAonCard"] {
        if let Some(valid) = p.get(key) {
            let more = selector_any(ctx, valid)?;
            match pattern.get_mut("what").and_then(Value::as_array_mut) {
                Some(what) => {
                    for pred in more {
                        if !what.contains(&pred) {
                            what.push(pred);
                        }
                    }
                }
                None => pattern["what"] = json!(more),
            }
        }
    }
    match p.get("IsSingleTarget") {
        None => {}
        Some("True") => match pattern.get_mut("what").and_then(Value::as_array_mut) {
            Some(what) => what.push(json!("SingleTarget")),
            None => pattern["what"] = json!(["SingleTarget"]),
        },
        Some(other) => return unmapped(format!("key:Trigger.SpellCast.IsSingleTarget={other}")),
    }
    // "Whenever you cast a spell that targets a creature you control": one of its targets matches.
    if let Some(valid) = p.get("TargetsValid") {
        let targeted = selector_any(ctx, valid)?;
        match pattern.get_mut("what").and_then(Value::as_array_mut) {
            Some(what) => what.push(json!({ "Targeting": targeted })),
            None => pattern["what"] = json!([{ "Targeting": targeted }]),
        }
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
    keys(p, "Drawn", &["ValidCard", "Number"])?;
    let who = match p.get("ValidCard") {
        Some("Card.YouOwn" | "Card.YouCtrl") => "You",
        Some("Card.OppOwn") => "Opponents",
        None | Some("Card") => "EachPlayer",
        Some(other) => return unmapped(format!("key:Trigger.Drawn.ValidCard={other}")),
    };
    let drew = json!({"Drew": {"who": who}});
    // "Your second card each turn".
    match p.get("Number").map(str::parse::<u32>) {
        None => Ok(drew),
        Some(Ok(nth)) if nth > 0 => Ok(json!({"Nth": {"event": drew, "nth": nth}})),
        Some(_) => unmapped("key:Trigger.Drawn.Number"),
    }
}

fn life_gained(p: &Params) -> Tr<Value> {
    keys(p, "LifeGained", &["ValidPlayer", "FirstTime"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    first_time(p, json!({"LifeGained": {"who": who}}))
}

/// `FirstTime$ True`: the first time in the turn the event happens to the player or object.
pub(super) fn first_time(p: &Params, event: Value) -> Tr<Value> {
    match p.get("FirstTime") {
        None | Some("False") => Ok(event),
        Some("True") => Ok(json!({"Nth": {"event": event, "nth": 1}})),
        Some(other) => unmapped(format!("key:Trigger.FirstTime={other}")),
    }
}

/// A trigger on a permanent: sacrificed, tapped, untapped, blocks, becomes blocked.
fn what_trigger(ctx: &Ctx, p: &Params, variant: &str) -> Tr<Value> {
    keys(p, variant, &["ValidCard"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    Ok(json!({ variant: {"what": what} }))
}

/// `AttackerBlocked`: an attacker becomes blocked, once. A blocker filter ("whenever this blocks a creature") reads as the
/// pattern's `by` only for a blocker that is one object, since `by` triggers once for each matching blocker.
fn attacker_blocked(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "BecomesBlocked", &["ValidCard", "ValidBlocker"])?;
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    let mut node = json!({ "what": what });
    if let Some(valid) = p.get("ValidBlocker") {
        if !matches!(
            valid,
            "Card.Self"
                | "Card.EquippedBy"
                | "Card.AttachedBy"
                | "Creature.AttachedBy"
                | "Creature.EquippedBy"
                | "Creature.EnchantedBy"
        ) {
            return unmapped("key:Trigger.BecomesBlocked.ValidBlocker");
        }
        node["by"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({ "BecomesBlocked": node }))
}

/// Whether the condition compares the turn's count of spells cast for equality or an upper bound: true when it was
/// checked as the spell was cast, but a later spell changes it by the time the ability resolves.
fn counts_the_cast_exactly(condition: &Value) -> bool {
    let compare = |value: &Value| {
        value.get("Compare").is_some_and(|c| {
            matches!(
                c.get("op").and_then(Value::as_str),
                Some("Eq" | "Le" | "Lt")
            ) && c
                .get("left")
                .is_some_and(|left| left.to_string().contains("\"event\":\"SpellCast\""))
        })
    };
    compare(condition)
        || condition
            .get("All")
            .and_then(Value::as_array)
            .is_some_and(|all| all.iter().any(compare))
}
