//! `ReplaceBy::RedirectUpTo` (CR 614.9): part of the damage is dealt to another player or permanent by the same source
//! and the rest as it was ("the next 1 damage that would be dealt to this creature this turn is dealt to target creature
//! you control instead"). The shield fixed the new recipient as it began and keeps what it has left across events, as a
//! prevention shield of an amount does.

use mtg_core::card::CardType;

use super::{Candidate, Item, Origin, Pass, Slot};
use crate::choose::Target;
use crate::combat::DamageTarget;
use crate::events::Event;
use crate::rules::Rule;
use crate::zones::Zone;

impl Pass<'_> {
    /// The recipient the shield of `candidate` redirects to, while it can still be dealt damage: a player in the game,
    /// or the same creature, planeswalker or battle on the battlefield (CR 400.7).
    fn r6f_recipient(&self, candidate: &Candidate) -> Option<DamageTarget> {
        let Origin::Shield(id) = candidate.origin else {
            return None;
        };
        let redirect = self.shields.iter().find_map(|effect| match &effect.rule {
            Rule::PreventDamage(shield) if effect.id == id => shield.redirect,
            _ => None,
        })?;
        match redirect {
            Target::Player(player) => self
                .state
                .is_alive(player)
                .then_some(DamageTarget::Player(player)),
            Target::Object(object) => self
                .state
                .object(object.id)
                .filter(|o| o.counter() == object.counter && o.zone() == Zone::Battlefield)
                .and_then(|o| crate::characteristics::characteristics(self.state, o.id()))
                .filter(|chars| {
                    [CardType::Creature, CardType::Planeswalker, CardType::Battle]
                        .into_iter()
                        .any(|kind| chars.types.contains(kind))
                })
                .map(|_| DamageTarget::Object(object.id)),
        }
    }

    /// Splits the damage event at `at`: up to `most` of it is dealt to the shield's recipient by the same source, the rest
    /// stays with its recipient. Both parts carry this effect as applied (CR 614.5). When the new recipient can no longer
    /// be dealt damage, or is the recipient already, the event stays whole and the shield keeps its amount.
    pub(super) fn r6f_redirect_up_to(
        &mut self,
        at: usize,
        candidate: &Candidate,
        most: u32,
        applied: Vec<Origin>,
    ) {
        let Item::One(Event::DamageDealt {
            source,
            target,
            amount,
            deathtouch,
            form,
            toxic,
        }) = self.slots[at].item.clone()
        else {
            self.slots[at].applied = applied;
            return;
        };
        let taken = most.min(amount);
        let recipient = self
            .r6f_recipient(candidate)
            .filter(|recipient| *recipient != target && taken > 0);
        let Some(recipient) = recipient else {
            self.slots[at].applied = applied;
            return;
        };
        if let Origin::Shield(id) = candidate.origin
            && let Some(left) = self.left.get_mut(&id)
        {
            *left = left.saturating_sub(taken);
            if !self.touched.contains(&id) {
                self.touched.push(id);
            }
        }
        let part = |target, amount| Slot {
            item: Item::One(Event::DamageDealt {
                source,
                target,
                amount,
                deathtouch,
                form,
                toxic,
            }),
            applied: applied.clone(),
            frozen: false,
        };
        let mut parts = vec![part(recipient, taken)];
        if amount > taken {
            parts.push(part(target, amount - taken));
        }
        self.slots.splice(at..=at, parts);
    }
}
