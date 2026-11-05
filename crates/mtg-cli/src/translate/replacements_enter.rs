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
    if let Some(clean) = clean {
        if clean.get("DB") != Some("Cleanup") || clean.get("ClearRemembered") != Some("True") {
            return unmapped("replacement:Moved-Reveal-shape");
        }
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
