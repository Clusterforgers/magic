//! The `Shield` an `Effect` object makes from an `Event$ Moved` replacement about the cards it remembers: "if that spell would
//! be put into your graveyard, exile it instead".

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use super::effects_obj::duration;
use crate::forge::script::Params;

/// What `ValidCard$` or `ValidLKI$` may say to name only the cards the `Effect` remembers.
const REMEMBERED: [&str; 3] = [
    "Card.IsRemembered",
    "Card.nonLand+IsRemembered",
    // The remembered card is the only one the effect lets the player cast, so "cast with the effect" names it.
    "Card.CastSa Spell.MayPlaySource",
];

/// The shield of one `Moved` replacement: a remembered card that would go from the stack to a graveyard goes to exile.
pub fn shield(
    ctx: &mut Ctx,
    line: &Params,
    p: &Params,
    remembered: Option<&Value>,
) -> Tr<Vec<Value>> {
    check_keys(
        p,
        "Effect.Replacement.Moved",
        &[
            "Event",
            "ValidCard",
            "ValidLKI",
            "Origin",
            "Destination",
            "ReplaceWith",
        ],
        |_, _| false,
    )?;
    let Some(who) = remembered else {
        return unmapped("replacement:Moved-shield-without-remembered");
    };
    let valid = p.get("ValidCard").or_else(|| p.get("ValidLKI"));
    if !valid.is_some_and(|word| REMEMBERED.contains(&word)) {
        return unmapped("replacement:Moved-shield-valid");
    }
    // The remembered card is cast, so the move is the spell leaving the stack; a line that names another origin is another rule.
    if !matches!(p.get("Origin"), None | Some("Stack")) || p.get("Destination") != Some("Graveyard")
    {
        return unmapped("replacement:Moved-shield-zones");
    }
    let Some(name) = p.get("ReplaceWith") else {
        return unmapped("replacement:Moved-without-ReplaceWith");
    };
    let part = ctx.sub(name)?;
    check_keys(
        &part,
        "Effect.Replacement.Moved",
        &["DB", "Defined", "Origin", "Destination"],
        |key, _| key == "Hidden",
    )?;
    let exiles = part.get("DB") == Some("ChangeZone")
        && part.get("Defined") == Some("ReplacedCard")
        && part.get("Destination") == Some("Exile")
        && matches!(part.get("Origin"), Some("Stack" | "All"));
    if !exiles {
        return unmapped("replacement:Moved-not-an-exile");
    }
    Ok(vec![json!({"Shield": {
        "when": {"ZoneChange": {"what": [{"Is": who}], "from": "Stack", "to": "Graveyard"}},
        "by": {"MoveTo": "Exile"},
        "duration": duration(line.get("Duration"))?,
    }})])
}
