//! Trigger modes and keys of round 13: counters put on several permanents at once, a clash won, and the controller
//! word of a card's own discard.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::keys;
use super::triggers_more::{batch, counters};
use crate::forge::script::Params;

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    Some(match mode {
        "CounterAddedAll" => counters_put_all(ctx, p),
        "Clashed" => clashed(p),
        _ => return None,
    })
}

/// "Whenever one or more +1/+1 counters are put on one or more Humans you control": `Valid$` is what receives them, and
/// the counters of one event trigger the ability once.
fn counters_put_all(ctx: &Ctx, p: &Params) -> Tr<Value> {
    let Some(valid) = p.get("Valid") else {
        return unmapped("key:Trigger.CounterAddedAll.Valid-missing");
    };
    let line = p.without(&["Valid"]).with("ValidCard", valid);
    counters(ctx, &line, "CountersPut").map(|inner| batch(&inner))
}

/// "Whenever you clash and win" (`Won$ True`) and "whenever you clash" (no `Won$`, any result); only a lost clash stays a gap.
fn clashed(p: &Params) -> Tr<Value> {
    keys(p, "Clashed", &["ValidPlayer", "Won"])?;
    let who = super::triggers::player_ref(p.get("ValidPlayer"))?;
    match p.get("Won") {
        Some("True") => Ok(json!({"W15cClashWon": {"who": who}})),
        None => Ok(json!({"Did": {"action": "Clashed", "who": who}})),
        _ => unmapped("trigger:Clashed"),
    }
}

/// The line of a card's own discard ("When you discard this card") without `TriggerController$ TriggeredCardController`:
/// only a card's owner discards it, so the controller of the ability is that player already.
pub fn g13c_own_discard(p: &Params, mode: &str) -> Params {
    if mode == "Discarded"
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("TriggerController") == Some("TriggeredCardController")
    {
        p.without(&["TriggerController"])
    } else {
        p.clone()
    }
}

/// The zone an action's object comes from (`Origin$`) as a test of the object: a land played from exile ("Whenever you
/// play a land from exile") arrived on the battlefield from there; a creature exiled from the battlefield was there in
/// the state before the exile, which the action is judged in. Any other mode or word is a gap.
pub fn g13c_origin(mode: &str, origin: &str) -> Tr<Value> {
    match (mode, origin) {
        ("LandPlayed", _) => played_from(origin),
        ("Exiled", "Battlefield") => Ok(json!({"In": "Battlefield"})),
        _ => unmapped(format!("key:Trigger.{mode}.Origin")),
    }
}

/// The zones a land is played from as `ArrivedFrom` tests; the ante zone is not in the game.
fn played_from(origin: &str) -> Tr<Value> {
    let mut zones = Vec::new();
    for word in origin.split(',') {
        match word {
            "Ante" => {}
            "Hand" | "Library" | "Graveyard" | "Exile" | "Command" => {
                zones.push(json!({ "ArrivedFrom": word }));
            }
            _ => return unmapped(format!("key:Trigger.LandPlayed.Origin={origin}")),
        }
    }
    Ok(match zones.as_slice() {
        [] => return unmapped(format!("key:Trigger.LandPlayed.Origin={origin}")),
        [only] => only.clone(),
        _ => json!({ "AnyOf": zones }),
    })
}

/// Two trigger lines of one printed ability ("When ~ enters or attacks, ...") that the script wrote apart: a decider on
/// one line only follows the printed words (a "you may" right after the trigger words asks on both, otherwise neither
/// asks), and lines of different events that differ only in their intervening condition ("Whenever an opponent casts a
/// spell during your turn or when ~ dies") stay two abilities, as no one event fits both. True when they stay apart.
pub fn g13c_settle(abilities: &mut [Value], description: Option<&str>) -> bool {
    let effects = |ability: &Value| ability.pointer("/Triggered/effects").cloned();
    if let [first, second] = &*abilities
        && let (Some(one), Some(two)) = (effects(first), effects(second))
    {
        let asked = unwrap_may(&one)
            .filter(|inner| *inner == two)
            .map(|_| one.clone())
            .or_else(|| {
                unwrap_may(&two)
                    .filter(|inner| *inner == one)
                    .map(|_| two.clone())
            });
        if let Some(asked) = asked {
            let printed_may = description
                .and_then(|text| text.split_once(", "))
                .is_some_and(|(_, step)| step.starts_with("you may "));
            let settled = if printed_may {
                asked
            } else {
                unwrap_may(&asked).unwrap_or(asked)
            };
            for ability in abilities.iter_mut() {
                if let Some(slot) = ability.pointer_mut("/Triggered/effects") {
                    *slot = settled.clone();
                }
            }
        }
    }
    apart_by_condition(abilities)
}

/// The steps inside an effect list that is one plain "you may" (`[{"May": {"effects": [...]}}]`).
fn unwrap_may(list: &Value) -> Option<Value> {
    let [only] = list.as_array()?.as_slice() else {
        return None;
    };
    let inner = only.get("May")?;
    if inner.as_object()?.len() != 1 {
        return None;
    }
    inner.get("effects").cloned()
}

/// Whether the abilities wait for events of different kinds and differ in nothing but their intervening condition.
fn apart_by_condition(abilities: &[Value]) -> bool {
    let kind = |ability: &Value| {
        ability
            .pointer("/Triggered/when")
            .and_then(Value::as_object)
            .and_then(|map| map.keys().next().cloned())
    };
    let rest = |ability: &Value| {
        let mut inner = ability.get("Triggered").cloned().unwrap_or(Value::Null);
        if let Some(map) = inner.as_object_mut() {
            map.remove("when");
            map.remove("if");
        }
        inner
    };
    let kinds: Vec<Option<String>> = abilities.iter().map(kind).collect();
    let Some(first) = abilities.first() else {
        return false;
    };
    abilities.len() > 1
        && kinds
            .iter()
            .all(|kind| kind.as_deref().is_some_and(|word| word != "AnyOf"))
        && kinds
            .iter()
            .enumerate()
            .all(|(at, kind)| !kinds[..at].contains(kind))
        && abilities.iter().all(|each| rest(each) == rest(first))
        && abilities
            .iter()
            .any(|each| each.pointer("/Triggered/if") != first.pointer("/Triggered/if"))
}
