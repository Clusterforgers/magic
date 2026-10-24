//! `Effect::Fight` (CR 701.14): each creature deals damage equal to its power to the other, once, if both are creatures
//! on the battlefield.

use mtg_cards::effect::Effect;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::combat::DamageTarget;
use crate::damage::{self, Dealer};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// A creature on the battlefield that fights: the object and the damage it deals, as the power it has now.
struct Fighter {
    id: ObjectId,
    damage: u32,
}

/// The creature `object` names if it is still that object, on the battlefield and a creature (CR 701.14b).
fn fighter(state: &GameState, object: ObjectRef) -> Option<Fighter> {
    let found = state
        .object(object.id)
        .filter(|o| o.counter() == object.counter && o.zone() == Zone::Battlefield)?;
    let chars = characteristics(state, found.id())?;
    chars.types.contains(CardType::Creature).then(|| Fighter {
        id: found.id(),
        damage: u32::try_from(chars.power.unwrap_or(0).max(0)).unwrap_or(0),
    })
}

/// Each creature `what` names fights each creature `with` names: both deal their damage in the same batch, so
/// deathtouch, lifelink, wither, infect and prevention apply as for any damage (it is not combat damage, CR 701.14c).
///
/// A creature that is gone or no longer a creature makes the fight deal nothing, and a reference that names nobody (a
/// target that was illegal, "up to one" chosen as none) is no fight. A creature fighting itself deals its damage to
/// itself twice.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Fight { what, with } = effect else {
        return Err(wrong_effect("Fight"));
    };
    let state = cx.state();
    let mut events: Vec<Event> = Vec::new();
    for first in cx.scope.objects(what)? {
        for second in cx.scope.objects(with)? {
            let (Some(a), Some(b)) = (fighter(state, first), fighter(state, second)) else {
                continue;
            };
            for (dealer, recipient, amount) in [(&a, &b, a.damage), (&b, &a, b.damage)] {
                events.extend(damage::dealt_by(
                    state,
                    &Dealer::live(state, dealer.id),
                    DamageTarget::Object(recipient.id),
                    amount,
                ));
            }
        }
    }
    Ok(Run::events(events))
}
