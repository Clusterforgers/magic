//! `Effect::DealDamage`: a source deals damage to players, creatures, planeswalkers and battles (CR 120).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The source deals the amount to each recipient still there to take it; damage of nothing is no event.
///
/// The source is the spell or the ability's source unless the effect names another. Recipients that left the
/// game, are not on the battlefield any more or cannot be dealt damage are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::DealDamage { source, to, amount } = effect else {
        return Err(wrong_effect("DealDamage"));
    };
    let amount = u32::try_from(cx.scope.amount(amount)?.max(0)).unwrap_or(u32::MAX);
    let source = match source {
        None => cx.scope.source.id,
        Some(named) => cx
            .scope
            .objects(named)?
            .first()
            .map(|object| object.id)
            .ok_or_else(|| {
                ResolveError::Invalid("the source of the damage is not an object".into())
            })?,
    };
    let mut events = Vec::new();
    if amount > 0 {
        for recipient in cx.scope.refs(to)? {
            let target = match recipient {
                Target::Player(player) if cx.state().is_alive(player) => {
                    DamageTarget::Player(player)
                }
                Target::Object(object) if cx.scope.can_be_dealt_damage(object) => {
                    DamageTarget::Object(object.id)
                }
                _ => continue,
            };
            events.push(Event::DamageDealt {
                source,
                target,
                amount,
            });
        }
    }
    Ok(Run::events(events))
}
