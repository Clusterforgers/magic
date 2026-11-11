//! "The next spell you cast this turn can't be countered" as Forge writes it: a delayed trigger on the caster's next spell
//! (`Mode$ SpellCast | Static$ True | ThisTurn$ True`) that makes an effect with a counter replacement of `CantHappen`
//! over the spell it remembers. It is the rule `GameRule::NextSpell` with the benefit `Uncounterable`.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// The keys the delayed trigger line carries.
const TRIGGER: [&str; 11] = [
    "SP",
    "AB",
    "DB",
    "Cost",
    "SubAbility",
    "Mode",
    "ValidCard",
    "ValidActivatingPlayer",
    "Execute",
    "ThisTurn",
    "Static",
];

/// Whether the delayed trigger `p` is the shape of a next spell that can't be countered: a static trigger on the next spell
/// of the caster this turn.
pub fn ep26_is_uncounterable_next(ctx: &Ctx, p: &Params) -> bool {
    p.get("Mode") == Some("SpellCast")
        && p.get("Static") == Some("True")
        && p.get("Execute")
            .and_then(|name| ctx.svar(name))
            .is_some_and(|line| line.contains("ReplacementEffects$"))
}

/// The rule of "the next spell you cast this turn can't be countered"; any other shape is a gap.
pub fn ep26_uncounterable_next(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    check_keys(p, "DelayedTrigger.SpellCast.Static", &TRIGGER, |_, _| false)?;
    if p.get("ThisTurn") != Some("True") || p.get("ValidActivatingPlayer") != Some("You") {
        return unmapped("key:DelayedTrigger.SpellCast.Static-not-yours-this-turn");
    }
    let execute = p.get("Execute").unwrap_or_default();
    let effect = ctx.sub(execute)?;
    check_keys(
        &effect,
        "DelayedTrigger.SpellCast.Static.Effect",
        &[
            "DB",
            "ReplacementEffects",
            "RememberObjects",
            "ForgetOnMoved",
        ],
        |_, _| false,
    )?;
    let remembers = effect.get("DB") == Some("Effect")
        && effect.get("RememberObjects") == Some("TriggeredCard")
        && effect.get("ForgetOnMoved") == Some("Stack");
    let Some(name) = effect
        .get("ReplacementEffects")
        .filter(|names| !names.contains(','))
    else {
        return unmapped("key:DelayedTrigger.SpellCast.Static-ReplacementEffects");
    };
    let replacement = ctx.sub(name.trim())?;
    check_keys(
        &replacement,
        "DelayedTrigger.SpellCast.Static.Replacement",
        &["Event", "ValidCard", "ValidSA", "Layer"],
        |_, _| false,
    )?;
    let cant_counter = replacement.get("Event") == Some("Counter")
        && replacement.get("ValidCard") == Some("Card.IsRemembered")
        && replacement.get("ValidSA") == Some("Spell")
        && replacement.get("Layer") == Some("CantHappen");
    if !remembers || !cant_counter {
        return unmapped("key:DelayedTrigger.SpellCast.Static-shape");
    }
    let valid = match p.get("ValidCard") {
        None | Some("Card") => "",
        Some(valid) => valid,
    };
    let mut rule = json!({"benefit": "Uncounterable"});
    if !valid.is_empty() {
        let what = selector_any(ctx, valid)?;
        rule["what"] = json!(what);
    }
    Ok(vec![
        json!({"Rule": {"rule": {"NextSpell": rule}, "duration": "UntilEndOfTurn"}}),
    ])
}
