//! Imprint beside remember on one step (e17b): a `DigUntil` that keeps the card found in one memory and the other cards
//! revealed in the other (`ImprintFound` with `RememberRevealed`, `RememberFound` with `ImprintRevealed`).

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::memory::Slot;
use crate::forge::script::Params;

/// Whether the line is a `DigUntil` that splits its cards over the two memories, which it binds itself.
pub fn splits(api: &str, p: &Params) -> bool {
    api == "DigUntil" && split(p).is_some()
}

/// The memory of the card found and the memory of the rest, when a `DigUntil` splits them over the two slots.
pub fn split(p: &Params) -> Option<(Slot, Slot)> {
    let on = |key| p.get(key) == Some("True");
    match (
        on("ImprintFound"),
        on("RememberRevealed"),
        on("RememberFound"),
        on("ImprintRevealed"),
    ) {
        (true, true, false, false) => Some((Slot::Imprinted, Slot::Remembered)),
        (false, false, true, true) => Some((Slot::Remembered, Slot::Imprinted)),
        _ => None,
    }
}

/// Binds the card found as it is after its move and the other revealed cards as they are after theirs: the found card
/// moved first, so the move of the revealed cards no longer reaches it (CR 400.7) and "the rest" are the others.
pub fn bind(
    ctx: &mut Ctx,
    (found_slot, rest_slot): (Slot, Slot),
    found_move: &mut Value,
    rest: &mut Value,
) -> Tr<()> {
    let found_verb = verb(found_move)?;
    let found = ctx.fresh("found_moved");
    found_move[found_verb]["as"] = json!(found);
    let rest_verb = verb(rest)?;
    let others = ctx.fresh("rest_moved");
    rest[rest_verb]["as"] = json!(others);
    ctx.counted.insert(others.clone());
    for (slot, name) in [(found_slot, found), (rest_slot, others)] {
        let held = Some(json!({ "Bound": name }));
        match slot {
            Slot::Remembered => ctx.remembered = held,
            Slot::Imprinted => ctx.imprinted = held,
        }
    }
    Ok(())
}

/// "Return target card ... to the battlefield. If a Hero enters this way, it enters with two additional +1/+1 counters on
/// it": an `Effect` remembering its target and itself with one `Moved` replacement that puts counters on a returned card
/// of a kind, followed by the one move of that target to the battlefield. The move becomes a choice on the card's kind,
/// read as it leaves its zone. `None` for any other line.
pub fn returned_with_counters(ctx: &mut Ctx, p: &Params, top: bool) -> Option<Tr<Vec<Value>>> {
    if p.get("DB").or_else(|| p.get("SP")) != Some("Effect")
        || p.get("RememberObjects") != Some("Targeted & Self")
    {
        return None;
    }
    Some(returned(ctx, p, top))
}

fn returned(ctx: &mut Ctx, p: &Params, top: bool) -> Tr<Vec<Value>> {
    let shape = || unmapped("key:Effect.returned-with-counters-shape");
    super::ctx::check_keys(
        p,
        "Effect",
        &[
            "SP",
            "DB",
            "RememberObjects",
            "ReplacementEffects",
            "ExileOnMoved",
            "SubAbility",
            "SpellDescription",
        ],
        |_, _| false,
    )?;
    // The effect ends as the card leaves its zone, so it reaches only this move.
    let (Some(name), Some(next), Some("Graveyard,Stack")) = (
        p.get("ReplacementEffects"),
        p.get("SubAbility"),
        p.get("ExileOnMoved"),
    ) else {
        return shape();
    };
    let replacement = ctx.sub(name)?;
    let kind = replacement
        .get("ValidCard")
        .and_then(|valid| valid.strip_suffix(".IsRemembered"));
    let (Some(kind), Some("Moved"), Some("Battlefield"), Some(put)) = (
        kind,
        replacement.get("Event"),
        replacement.get("Destination"),
        replacement.get("ReplaceWith"),
    ) else {
        return shape();
    };
    let put = ctx.sub(put)?;
    let count = put.get("CounterNum").and_then(|n| n.parse::<u32>().ok());
    let counter = put.get("CounterType").and_then(super::amount::counter_kind);
    let (Some(count), Some(counter), Some("PutCounter"), Some("ReplacedCard"), Some("True")) = (
        count,
        counter,
        put.get("DB"),
        put.get("Defined"),
        put.get("ETB"),
    ) else {
        return shape();
    };
    let back = ctx.sub(next)?;
    if back.get("DB") != Some("ChangeZone")
        || back.get("Destination") != Some("Battlefield")
        || !back.has("ValidTgts")
        || back.has("SubAbility")
    {
        return shape();
    }
    let mut effects = super::effects::chain_at(ctx, &back, top)?;
    let [step] = effects.as_mut_slice() else {
        return shape();
    };
    let Some(what) = step
        .get("Move")
        .filter(|m| m["to"].get("Battlefield").is_some() && m["what"].get("Target").is_some())
        .map(|m| m["what"].clone())
    else {
        return shape();
    };
    let plain = step.clone();
    let mut with = step.clone();
    with["Move"]["to"]["Battlefield"]["counters"] = json!([[counter, count]]);
    let selector = super::valid::selector_any(ctx, kind)?;
    let condition =
        json!({"CountOf": {"subject": what, "selector": selector, "op": "Ge", "right": 1}});
    Ok(vec![
        json!({"If": {"condition": condition, "then": [with], "otherwise": [plain]}}),
    ])
}

/// The verb of a move step.
fn verb(step: &Value) -> Tr<&'static str> {
    match step {
        _ if step.get("Exile").is_some() => Ok("Exile"),
        _ if step.get("Move").is_some() => Ok("Move"),
        _ => unmapped("key:DigUntil.split-memory-hidden-move"),
    }
}
