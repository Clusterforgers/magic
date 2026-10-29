//! Every player taking back what they own: "Each player gains control of all creatures they own." is a pass for each
//! player that gives them, for good, the creatures they own.

use super::selectors::all;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref};

/// The name the player of the pass is bound to.
pub(super) const W20C_PLAYER: &str = "player";

/// "each player gains control of all creatures they own": the creatures of a group, each to its owner.
pub(super) fn w20c_each_owner(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need("each player gains control of")?;
    let Ref::Each(mut group) = all(r)? else {
        return None;
    };
    r.need("they own")?;
    let player = Ref::Bound(W20C_PLAYER.to_owned());
    // After the card types and before the other parts, as the cards that hand-written files model it.
    let at = group
        .0
        .iter()
        .take_while(|pred| matches!(pred, Pred::Type(_)))
        .count();
    group.0.insert(at, Pred::OwnedBy(player.clone()));
    Some(vec![Effect::RepeatEach {
        over: Ref::EachPlayer,
        bind: W20C_PLAYER.to_owned(),
        effects: vec![Effect::Continuous {
            affected: Ref::Each(group),
            mods: vec![Modification::SetController(player)],
            duration: Duration::Forever,
        }],
    }])
}
