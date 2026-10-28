//! A sentence done for each player or opponent: "For each opponent, you create a 1/1 white Human Soldier creature token."
//! is a `RepeatEach` over the opponents. The steps name no player of the pass: "that player" in them is the player of
//! the event of the ability, which the pass does not bind, so a step that says it stays unread.

use super::conditional::body;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::Ref;

/// The name the player of a pass is bound to: "opponent" or "player".
pub fn bound_name(over: &Ref) -> Option<&'static str> {
    match over {
        Ref::Opponents => Some("opponent"),
        Ref::EachPlayer => Some("player"),
        _ => None,
    }
}

/// "For each opponent, S" or "For each player, S": the steps once for each of them, in turn order.
pub fn each_player(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("for each")?;
    let over = if r.word("opponent") {
        Ref::Opponents
    } else {
        r.need("player")?;
        Ref::EachPlayer
    };
    r.punct(',').then_some(())?;
    if let Some(effects) = super::c10_dice_targets::c10_dice_per_opponent(r, &over) {
        return Some(effects);
    }
    let (event, targets) = (
        std::mem::replace(&mut r.that_player, false),
        r.targets.len(),
    );
    let steps = body(r);
    r.that_player = event;
    // One target would serve every pass; a target for each player is another construct.
    (r.targets.len() == targets).then_some(())?;
    Some(vec![Effect::RepeatEach {
        bind: bound_name(&over)?.to_owned(),
        over,
        effects: steps?,
    }])
}
