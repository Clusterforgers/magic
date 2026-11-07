//! `Event$ Moved` replacements of the card itself that ask the controller something as it enters: reveal a card or enter
//! tapped (unless a permanent is already controlled).

use serde_json::{Value, json};

use super::amount::amount;
use super::condition::tapped_when;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::valid::selector_any;
use crate::forge::script::Params;

/// "As it enters, you may reveal a Dragon card from your hand. It enters tapped unless you revealed one or you control
/// a Dragon": the reveal, and the condition under which it is asked (no such permanent is controlled).
pub fn reveal_or_tapped(ctx: &mut Ctx, part: &Params) -> Tr<(Value, Value)> {
    check_keys(
        part,
        "Replacement",
        &[
            "DB",
            "RevealValid",
            "Optional",
            "RememberRevealed",
            "SubAbility",
        ],
        |_, _| false,
    )?;
    let (Some(valid), Some("True"), Some("True"), Some(next)) = (
        part.get("RevealValid"),
        part.get("Optional"),
        part.get("RememberRevealed"),
        part.get("SubAbility"),
    ) else {
        return unmapped("replacement:Moved-Reveal-shape");
    };
    let tap = ctx.sub(next)?;
    check_keys(
        &tap,
        "Replacement",
        &[
            "DB",
            "ETB",
            "Defined",
            "ConditionCheckSVar",
            "ConditionSVarCompare",
            "SubAbility",
        ],
        |_, _| false,
    )?;
    if tap.get("DB") != Some("Tap")
        || tap.get("ETB") != Some("True")
        || tap.get("Defined") != Some("Self")
        || tap.get("ConditionSVarCompare") != Some("EQ0")
    {
        return unmapped("replacement:Moved-Reveal-shape");
    }
    let clean = tap
        .get("SubAbility")
        .map(|name| ctx.sub(name))
        .transpose()?;
    if let Some(clean) = clean
        && (clean.get("DB") != Some("Cleanup") || clean.get("ClearRemembered") != Some("True"))
    {
        return unmapped("replacement:Moved-Reveal-shape");
    }
    // The count is the revealed cards plus the permanents controlled, so zero means neither.
    let held = tap
        .get("ConditionCheckSVar")
        .and_then(|name| ctx.svar(name))
        .and_then(|text| text.strip_prefix("Count$RememberedSize/Plus."))
        .map(str::to_owned);
    let Some(held) = held else {
        return unmapped("replacement:Moved-Reveal-presence");
    };
    let controlled = amount(ctx, &held)?;
    let mut selector = selector_any(ctx, valid)?;
    selector.push(json!({"In": "Hand"}));
    selector.push(json!({"OwnedBy": "You"}));
    let condition = tapped_when(json!({"Compare": {"left": controlled, "op": "Eq", "right": 0}}));
    Ok((json!({ "RevealOrTapped": selector }), condition))
}

/// "If ~ would enter, sacrifice two untapped lands instead. If you do, put ~ onto the battlefield. If you don't, put it
/// into its owner's graveyard" and Mox Diamond's "you may discard a land card instead": a `Sacrifice` or `Discard` that
/// remembers what it took, then the two `ChangeZone`s of the replaced card that compare the count with what the cost
/// takes. `None` for another chain.
pub fn e14b_pay_or_graveyard(
    ctx: &mut Ctx,
    p: &Params,
    condition: Option<&Value>,
) -> Option<Tr<Value>> {
    let first = ctx.svar(p.get("ReplaceWith")?).map(Params::parse)?;
    let remembers = match first.get("DB")? {
        "Sacrifice" => "RememberSacrificed",
        "Discard" => "RememberDiscarded",
        _ => return None,
    };
    first.get(remembers)?;
    Some(pay_or_graveyard(ctx, p, &first, condition))
}

fn pay_or_graveyard(
    ctx: &mut Ctx,
    p: &Params,
    first: &Params,
    condition: Option<&Value>,
) -> Tr<Value> {
    let shape = || unmapped("replacement:Moved-PayOrGraveyard-shape");
    if condition.is_some()
        || p.get("Destination") != Some("Battlefield")
        || p.has("Origin")
        || p.has("ActiveZones")
    {
        return shape();
    }
    let (cost, optional, count) = if first.get("DB") == Some("Sacrifice") {
        check_keys(
            first,
            "Replacement",
            &[
                "DB",
                "SacValid",
                "Defined",
                "RememberSacrificed",
                "Amount",
                "StrictAmount",
                "SubAbility",
            ],
            |_, _| false,
        )?;
        let Ok(count) = first.get("Amount").unwrap_or("1").parse::<u32>() else {
            return unmapped("replacement:Moved-PayOrGraveyard-amount");
        };
        if first.get("Defined") != Some("You")
            || (count > 1 && first.get("StrictAmount") != Some("True"))
        {
            return shape();
        }
        let Some(valid) = first.get("SacValid") else {
            return shape();
        };
        let mut sacrifice = json!({"filter": selector_any(ctx, valid)?});
        if count > 1 {
            sacrifice["count"] = json!(count);
        }
        (json!([{ "Sacrifice": sacrifice }]), false, count)
    } else {
        check_keys(
            first,
            "Replacement",
            &[
                "DB",
                "DiscardValid",
                "Mode",
                "RememberDiscarded",
                "Optional",
                "AILogic",
                "SubAbility",
            ],
            |_, _| false,
        )?;
        let (Some(valid), Some("TgtChoose"), Some("True")) = (
            first.get("DiscardValid"),
            first.get("Mode"),
            first.get("Optional"),
        ) else {
            return shape();
        };
        (
            json!([{ "Discard": { "filter": selector_any(ctx, valid)? } }]),
            true,
            1,
        )
    };
    // The graveyard when fewer than the cost were taken, the battlefield when all were, then the cleanup.
    let (fewer, all) = if count == 1 {
        ("EQ0".to_owned(), "EQ1".to_owned())
    } else {
        (format!("LT{count}"), format!("GE{count}"))
    };
    let Some(next) = first.get("SubAbility") else {
        return shape();
    };
    let grave = ctx.sub(next)?;
    let Some(next) = grave.get("SubAbility") else {
        return shape();
    };
    let field = ctx.sub(next)?;
    for (part, to, compare) in [(&grave, "Graveyard", &fewer), (&field, "Battlefield", &all)] {
        check_keys(
            part,
            "Replacement",
            &[
                "DB",
                "Origin",
                "Destination",
                "Defined",
                "ConditionCheckSVar",
                "ConditionSVarCompare",
                "SubAbility",
            ],
            |_, _| false,
        )?;
        let counted = part
            .get("ConditionCheckSVar")
            .and_then(|name| ctx.svar(name))
            .map(str::trim);
        if part.get("DB") != Some("ChangeZone")
            || part.get("Origin") != Some("All")
            || part.get("Destination") != Some(to)
            || part.get("Defined") != Some("ReplacedCard")
            || part.get("ConditionSVarCompare") != Some(compare.as_str())
            || counted != Some("Remembered$Amount")
        {
            return shape();
        }
    }
    if let Some(next) = field.get("SubAbility") {
        let clean = ctx.sub(next)?;
        if clean.get("DB") != Some("Cleanup")
            || clean.get("ClearRemembered") != Some("True")
            || clean.has("SubAbility")
        {
            return shape();
        }
    }
    let mut by = json!({ "cost": cost });
    if optional {
        by["optional"] = json!(true);
    }
    Ok(json!({"Replacement": {
        "when": {"Enters": {"what": ["This"]}},
        "by": {"PayOrGraveyard": by}
    }}))
}
