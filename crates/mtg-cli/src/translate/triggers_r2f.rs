//! Trigger modes read from one table row each, on events the engine already has: exploit and expend.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use super::triggers::{keys, player_ref};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The pattern of a mode this module reads; `None` for any other mode.
pub fn pattern(ctx: &Ctx, p: &Params, mode: &str) -> Option<Tr<Value>> {
    Some(match mode {
        "Exploited" => exploited(ctx, p),
        "ManaExpend" => expended(p),
        "BecomeMonarch" => monarch(p),
        "DayTimeChanges" => keys(p, mode, &[]).map(|()| json!({"DayNightChanges": {}})),
        "ClassLevelGained" => class_level(ctx, p),
        "PhaseIn" => object_event(ctx, p, "PhaseIn", "PhasedIn"),
        "PhaseOut" => object_event(ctx, p, "PhaseOut", "PhasedOut"),
        "SpellCopy" => spell_copied(ctx, p),
        "RolledDieOnce" => player_event(p, mode, "DiceRolled", &["ValidPlayer"]),
        "FlippedCoin" => coin_flip(p),
        _ => return None,
    })
}

/// "Whenever you roll one or more dice": the player who rolls, once for each roll.
fn player_event(p: &Params, mode: &str, variant: &str, read: &[&str]) -> Tr<Value> {
    keys(p, mode, read)?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({ variant: {"who": who} }))
}

/// "Whenever you win a coin flip"; losing one has no event of its own.
fn coin_flip(p: &Params) -> Tr<Value> {
    keys(p, "FlippedCoin", &["ValidPlayer", "ValidResult"])?;
    match p.get("ValidResult") {
        Some("Win") => player_event(
            p,
            "FlippedCoin",
            "CoinFlipWon",
            &["ValidPlayer", "ValidResult"],
        ),
        other => unmapped(format!(
            "key:Trigger.FlippedCoin.ValidResult={}",
            other.unwrap_or_default()
        )),
    }
}

/// "Whenever you become the monarch": the player who takes the crown. The turn-start words of a line stay gaps.
fn monarch(p: &Params) -> Tr<Value> {
    keys(p, "BecomeMonarch", &["ValidPlayer"])?;
    let who = player_ref(p.get("ValidPlayer"))?;
    Ok(json!({"BecomesMonarch": {"who": who}}))
}

/// "When this Class becomes level 2": the Class that gains the level and the level.
fn class_level(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "ClassLevelGained", &["ClassLevel", "ValidCard"])?;
    let Some(level) = p
        .get("ClassLevel")
        .and_then(|word| word.parse::<u32>().ok())
        .filter(|level| *level >= 2)
    else {
        return unmapped("key:Trigger.ClassLevelGained.ClassLevel");
    };
    let what = selector_any(ctx, p.get("ValidCard").unwrap_or("Card"))?;
    Ok(json!({"BecomesLevel": {"what": what, "level": level}}))
}

/// A permanent phases in or out: the objects the `ValidCard$` word names.
fn object_event(ctx: &Ctx, p: &Params, mode: &str, variant: &str) -> Tr<Value> {
    keys(p, mode, &["ValidCard"])?;
    // The permanent is judged as it was before it phased out, so the card itself is "phased out" only as a word of the line.
    let word = p.get("ValidCard").unwrap_or("Permanent");
    let what = selector_any(ctx, &word.replace(".phasedOutSelf", ".Self"))?;
    Ok(json!({ variant: {"what": what} }))
}

/// "Whenever you copy an instant spell": a spell is copied, by the player the line names.
fn spell_copied(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "SpellCopy", &["ValidCard", "ValidActivatingPlayer"])?;
    let by = player_ref(p.get("ValidActivatingPlayer"))?;
    let mut node = json!({ "by": by });
    if let Some(valid) = p.get("ValidCard") {
        node["what"] = json!(selector_any(ctx, valid)?);
    }
    Ok(json!({"SpellCopied": node}))
}

/// "When this creature exploits a creature": the source is the exploiting creature, and the creature sacrificed may be any
/// creature (the engine's event holds no object for the one sacrificed, so a narrower word stays a gap).
fn exploited(ctx: &Ctx, p: &Params) -> Tr<Value> {
    keys(p, "Exploited", &["ValidCard", "ValidSource"])?;
    if let Some(word) = p.get("ValidCard").filter(|word| *word != "Creature") {
        return unmapped(format!("key:Trigger.Exploited.ValidCard={word}"));
    }
    let what = selector_any(ctx, p.get("ValidSource").unwrap_or("Creature"))?;
    Ok(json!({"Exploits": {"what": what}}))
}

/// "Whenever you expend 4": the player's fourth total mana spent on spells in the turn.
fn expended(p: &Params) -> Tr<Value> {
    keys(p, "ManaExpend", &["Amount", "Player"])?;
    let who = player_ref(p.get("Player"))?;
    let Some(n) = p
        .get("Amount")
        .and_then(|word| word.parse::<u32>().ok())
        .filter(|n| *n > 0)
    else {
        return unmapped("key:Trigger.ManaExpend.Amount");
    };
    Ok(json!({"Expend": {"who": who, "n": n}}))
}
