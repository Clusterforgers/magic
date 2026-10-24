//! `Effect::DealDamage`: a source deals damage to players, creatures, planeswalkers and battles (CR 120).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::damage::{self, Dealer};
use crate::resolve::{Cx, ResolveError, Run};

/// The source deals each recipient still there to take it its amount; damage of nothing is no event.
///
/// The source is the spell or the ability's source unless the effect names another. A divided amount gives each
/// target its own share or an even part of the whole (`Scope::amounts_for`); a target that is not legal at
/// resolution has none. Recipients that left the game, are not on the battlefield any more or cannot be dealt
/// damage are passed over. A source with lifelink gains its controller the damage in the same batch
/// (`damage::dealt`).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::DealDamage { source, to, amount } = effect else {
        return Err(wrong_effect("DealDamage"));
    };
    let source = match source {
        None => cx.scope.source,
        Some(named) => match cx.scope.objects(named)?.first().copied() {
            Some(object) => object,
            // A creature that is no longer a legal target deals no damage; the spell does the rest (CR 608.2b).
            None => return Ok(Run::events(Vec::new())),
        },
    };
    let recipients = cx.scope.refs(to)?;
    let amounts = cx.scope.amounts_for(amount, &recipients)?;
    // A source that has left deals its damage as it was: with the keywords and under the controller it had.
    let dealer = match cx.scope.snapshot_of(source) {
        Some(shot) => Dealer::known(shot),
        None => Dealer::live(cx.state(), source.id),
    };
    let mut events = Vec::new();
    for (recipient, amount) in recipients.into_iter().zip(amounts) {
        let amount = u32::try_from(amount.max(0)).unwrap_or(u32::MAX);
        if amount == 0 {
            continue;
        }
        let target = match recipient {
            Target::Player(player) if cx.state().is_alive(player) => DamageTarget::Player(player),
            Target::Object(object) if cx.scope.can_be_dealt_damage(object) => {
                DamageTarget::Object(object.id)
            }
            _ => continue,
        };
        events.extend(damage::dealt_by(cx.state(), &dealer, target, amount));
    }
    Ok(Run::events(events))
}
