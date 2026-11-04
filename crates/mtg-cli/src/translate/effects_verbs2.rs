//! Effects that read sub-abilities or card faces: `GenericChoice` between two parts, `Branch` on a number and `SetState`.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{Ctx, Tr, only, unmapped};
use super::effects::chain_at;
use super::valid::{cmp, defined, subject};
use crate::forge::script::Params;

/// The effects of a verb this module reads; `None` for any other API.
pub fn effects(ctx: &mut Ctx, p: &Params, api: &str) -> Option<Tr<Vec<Value>>> {
    let node = match api {
        "GenericChoice" => generic_choice(ctx, p),
        "Branch" => branch(ctx, p),
        "SetState" => set_state(ctx, p),
        "VillainousChoice" => villainous_choice(ctx, p),
        _ => return None,
    };
    Some(node.map(|node| vec![node]))
}

/// `VillainousChoice`: the player `Defined$` names (an opponent, a chosen or remembered player) chooses one of the two
/// parts of `Choices$`.
fn villainous_choice(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "VillainousChoice", &["Choices", "Defined"])?;
    let Some(word) = p.get("Defined") else {
        return unmapped("key:VillainousChoice.no-Defined");
    };
    let who = defined(ctx, word)?;
    // Exactly one player faces the choice: a group of opponents is not one.
    if matches!(who.as_str(), Some("Opponents" | "EachPlayer")) {
        return unmapped(format!("key:VillainousChoice.Defined={word}"));
    }
    let names: Vec<&str> = p.get("Choices").unwrap_or_default().split(',').collect();
    let [first, second] = names[..] else {
        return unmapped("key:VillainousChoice.Choices");
    };
    let first = part(ctx, first)?;
    let second = part(ctx, second)?;
    Ok(json!({"VillainousChoice": {"who": who, "first": first, "second": second}}))
}

/// The effects of the named part.
fn part(ctx: &mut Ctx, name: &str) -> Tr<Vec<Value>> {
    let sub = ctx.sub(name)?;
    chain_at(ctx, &sub, false)
}

/// `GenericChoice` made by the controller between exactly two parts, as `Either`.
fn generic_choice(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(p, "GenericChoice", &["Choices", "Defined", "ValidTgts"])?;
    if !matches!(p.get("Defined"), None | Some("You")) {
        return unmapped("key:GenericChoice.Defined");
    }
    // A target is declared for the parts to read; without `Defined$ You` Forge lets the target choose, which is not read.
    if p.has("ValidTgts") {
        if p.get("Defined") != Some("You") {
            return unmapped("key:GenericChoice.ValidTgts-chooser");
        }
        subject(ctx, p, "You", None)?;
    }
    let names: Vec<&str> = p.get("Choices").unwrap_or_default().split(',').collect();
    let [first, second] = names[..] else {
        return unmapped("key:GenericChoice.Choices");
    };
    let first = part(ctx, first)?;
    let second = part(ctx, second)?;
    Ok(json!({"Either": {"first": first, "second": second}}))
}

/// `Branch` on a named number against a comparison (at least one by default), as `If`.
fn branch(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "Branch",
        &[
            "BranchConditionSVar",
            "BranchConditionSVarCompare",
            "TrueSubAbility",
            "FalseSubAbility",
        ],
    )?;
    let (Some(name), Some(yes)) = (p.get("BranchConditionSVar"), p.get("TrueSubAbility")) else {
        return unmapped("key:Branch.shape");
    };
    let word = p.get("BranchConditionSVarCompare").unwrap_or("GE1");
    let Some(op) = word.get(..2).and_then(cmp) else {
        return unmapped(format!("compare:{word}"));
    };
    let left = amount(ctx, name)?;
    let right = amount(ctx, word.get(2..).unwrap_or_default())?;
    let then = part(ctx, yes)?;
    let mut node = json!({"If": {
        "condition": {"Compare": {"left": left, "op": op, "right": right}},
        "then": then,
    }});
    if let Some(no) = p.get("FalseSubAbility") {
        node["If"]["otherwise"] = Value::Array(part(ctx, no)?);
    }
    Ok(node)
}

/// `SetState` with `Mode$ Transform`; the other modes (face up, flip, specific faces) are gaps.
fn set_state(ctx: &mut Ctx, p: &Params) -> Tr<Value> {
    only(
        p,
        "SetState",
        &["Mode", "Defined", "ValidTgts", "TargetMin", "TargetMax"],
    )?;
    if p.get("Mode") != Some("Transform") {
        return unmapped("key:SetState.Mode");
    }
    let what = subject(ctx, p, "Self", None)?;
    Ok(json!({"Transform": {"what": what}}))
}
