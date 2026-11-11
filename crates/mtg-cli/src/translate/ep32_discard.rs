//! "If a spell or ability an opponent controls causes you to discard ~, put it onto the battlefield [with two +1/+1
//! counters on it] instead of putting it into your graveyard." (`Event$ Moved | Discard$ True | ValidCause$
//! SpellAbility.OppCtrl`, from the hand): the card's own discard caused by an opponent, replaced by its move to the
//! battlefield.

use serde_json::{Value, json};

use super::amount::counter_kind;
use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The lines about the card's own move that read a key of their own: entering under an opponent's control
/// (`Layer$ Control`) and a discard by an opponent (`Discard$`); `None` for any other line.
pub fn ep32_own_moves(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Option<Tr<Value>> {
    super::ep28_under::ep28_under_opponent(ctx, p, condition)
        .or_else(|| ep32_discarded(ctx, p, condition))
}

/// The line of a card's discard by an opponent; `None` for a line that is no discard.
pub fn ep32_discarded(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Option<Tr<Value>> {
    p.has("Discard").then(|| discarded(ctx, p, condition))
}

fn discarded(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.Moved",
        &[
            "Event",
            "ActiveZones",
            "ValidCard",
            "ValidCause",
            "Discard",
            "EffectOnly",
            "ReplaceWith",
            "Description",
        ],
        |_, _| false,
    )?;
    let shape = condition.is_none()
        && p.get("ActiveZones") == Some("Hand")
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("ValidCause") == Some("SpellAbility.OppCtrl")
        && p.get("Discard") == Some("True")
        && p.get("EffectOnly") == Some("True");
    if !shape {
        return unmapped("replacement:Moved-discard-shape");
    }
    let part = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    check_keys(
        &part,
        "Replacement.Discard",
        &[
            "DB",
            "Defined",
            "Origin",
            "Destination",
            "WithCountersType",
            "WithCountersAmount",
        ],
        |_, _| false,
    )?;
    let enters = part.get("DB") == Some("ChangeZone")
        && part.get("Defined") == Some("ReplacedCard")
        && part.get("Origin") == Some("Hand")
        && part.get("Destination") == Some("Battlefield");
    if !enters {
        return unmapped("replacement:Moved-discard-not-onto-the-battlefield");
    }
    let mut battlefield = json!({});
    if let Some(kind) = part.get("WithCountersType") {
        let Some(counter) = counter_kind(kind) else {
            return unmapped(format!("key:Replacement.Discard.WithCountersType={kind}"));
        };
        let count = match part.get("WithCountersAmount").unwrap_or("1").parse::<u32>() {
            Ok(count) if count > 0 => count,
            _ => return unmapped("key:Replacement.Discard.WithCountersAmount"),
        };
        battlefield["counters"] = json!([[counter, count]]);
    } else if part.has("WithCountersAmount") {
        return unmapped("key:Replacement.Discard.WithCountersAmount");
    }
    Ok(json!({"Replacement": {
        "when": {"Discarded": {"what": ["This"], "by": "Opponents"}},
        "by": {"Instead": [{"Move": {"what": "This", "to": {"Battlefield": battlefield}}}]},
        "from": "Hand"
    }}))
}
