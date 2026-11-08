//! "{2}, Exile this card from your hand: Target land gains "{T}: Add {C}{C}" until this card is cast from exile. You may
//! cast this card for as long as it remains exiled." (the Streets of New Capenna hideaway lands, Emrakul, the Exigent
//! Doom): an `Effect` line that remembers its target and the card, grants the target an ability until the card is cast
//! from exile, and lets its controller cast the card from exile.

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The effects of the line, `None` for any other `Effect` line.
pub fn effects(ctx: &mut Ctx, p: &Params) -> Option<Tr<Vec<Value>>> {
    // `ImprintCards$ Self` is read off the line before it comes here (`memory.rs`).
    (p.get("RememberObjects") == Some("Targeted & Self")).then(|| read(ctx, p))
}

/// The two parts of the line, or the gap that stops them.
fn read(ctx: &mut Ctx, p: &Params) -> Tr<Vec<Value>> {
    let shape = || unmapped("key:Effect.until-cast-from-exile-shape");
    // The card the effect follows is the one its own cost exiled from the hand.
    if p.get("Duration") != Some("Permanent")
        || p.get("ForgetOnMoved") != Some("Exile")
        || p.get("Triggers") != Some("Cast")
        || p.get("ActivationZone") != Some("Hand")
        || !p
            .get("Cost")
            .is_some_and(|cost| cost.contains("ExileFromHand<1/CARDNAME>"))
        || !p.has("ValidTgts")
    {
        return shape();
    }
    if !ctx.oracle.contains(" is cast from exile. You may cast ")
        || !ctx.oracle.contains(" for as long as it remains exiled.")
    {
        return unmapped("key:Effect.until-cast-from-exile-wording");
    }
    // The static trigger moves the effect object along once the card is cast; the duration of the grant does that here.
    let cast = ctx.sub("Cast")?;
    if cast.get("Mode") != Some("SpellCast")
        || cast.get("ValidCard") != Some("Card.IsImprinted+IsRemembered+wasCastFromExile")
    {
        return shape();
    }
    let Some((grant, permission)) = p
        .get("StaticAbilities")
        .and_then(|names| names.split_once(','))
    else {
        return shape();
    };
    let grant = ctx.sub(grant.trim())?;
    let permission = ctx.sub(permission.trim())?;
    if grant.get("Mode") != Some("Continuous")
        || grant.get("Affected") != Some("Card.IsRemembered+!IsImprinted")
        || grant.get("AffectedZone") != Some("Battlefield")
        || permission.get("MayPlay") != Some("True")
        || permission.get("Affected") != Some("Card.IsImprinted+IsRemembered")
        || permission.get("AffectedZone") != Some("Exile")
    {
        return shape();
    }
    let Some(names) = grant.get("AddAbility") else {
        return shape();
    };
    let mods = super::statics::granted(ctx, names, false)?;
    let land = super::valid::subject(ctx, p, "", None)?;
    Ok(vec![
        json!({"Continuous": {"affected": land, "mods": mods, "duration": "UntilCastFromExile"}}),
        // The card the cost exiled is the source one zone change on; the permission names it alone, so it ends when the
        // card leaves exile (CR 400.7).
        json!({"Rule": {
            "rule": {"MayCast": {"only": {"Arrived": {"of": "This", "zone": "Exile"}}, "from": "Exile"}},
            "duration": "Forever"
        }}),
    ])
}
