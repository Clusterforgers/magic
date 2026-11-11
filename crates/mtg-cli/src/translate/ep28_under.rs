//! "~ enters under the control of an opponent of your choice" (`Event$ Moved | Layer$ Control`, a `ChoosePlayer` of an
//! opponent and a `ChangeZone` that gives that player control): `ReplaceBy::UnderChosenOpponent` of the card itself.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, check_keys, unmapped};
use crate::forge::script::Params;

/// The line of the card entering under a chosen opponent's control; `None` for a line of another layer.
pub fn ep28_under_opponent(
    ctx: &mut Ctx,
    p: &Params,
    condition: Option<&Value>,
) -> Option<Tr<Value>> {
    (p.get("Layer") == Some("Control")).then(|| under(ctx, p, condition))
}

fn under(ctx: &mut Ctx, p: &Params, condition: Option<&Value>) -> Tr<Value> {
    check_keys(
        p,
        "Replacement.Moved",
        &[
            "Event",
            "ValidCard",
            "Destination",
            "ReplaceWith",
            "Layer",
            "Description",
        ],
        |_, _| false,
    )?;
    let shape = condition.is_none()
        && p.get("ValidCard") == Some("Card.Self")
        && p.get("Destination") == Some("Battlefield")
        && p.get("Description").map(str::trim)
            == Some("CARDNAME enters under the control of an opponent of your choice.");
    if !shape {
        return unmapped("key:Replacement.Moved.Layer=Control");
    }
    let choose = ctx.sub(p.get("ReplaceWith").unwrap_or_default())?;
    check_keys(
        &choose,
        "Replacement.Control",
        &[
            "DB",
            "Defined",
            "Choices",
            "ChoiceTitle",
            "AILogic",
            "SubAbility",
        ],
        |_, _| false,
    )?;
    let chooses_opponent = choose.get("DB") == Some("ChoosePlayer")
        && matches!(choose.get("Defined"), None | Some("You"))
        && choose.get("Choices") == Some("Player.Opponent");
    if !chooses_opponent {
        return unmapped("key:Replacement.Moved.Layer=Control");
    }
    let moved = ctx.sub(choose.get("SubAbility").unwrap_or_default())?;
    check_keys(
        &moved,
        "Replacement.Control",
        &[
            "DB",
            "Hidden",
            "Origin",
            "Destination",
            "Defined",
            "GainControl",
            "SubAbility",
        ],
        |_, _| false,
    )?;
    let gives_control = moved.get("DB") == Some("ChangeZone")
        && moved.get("Destination") == Some("Battlefield")
        && moved.get("Defined") == Some("ReplacedCard")
        && moved.get("GainControl") == Some("ChosenPlayer");
    if !gives_control {
        return unmapped("key:Replacement.Moved.Layer=Control");
    }
    // Only a cleanup of the chosen player may follow.
    if let Some(rest) = moved.get("SubAbility") {
        let cleanup = ctx.sub(rest)?;
        if cleanup.get("DB") != Some("Cleanup") || cleanup.get("SubAbility").is_some() {
            return unmapped("key:Replacement.Moved.Layer=Control");
        }
    }
    Ok(
        json!({"Replacement": {"when": {"Enters": {"what": ["This"]}}, "by": "UnderChosenOpponent"}}),
    )
}
