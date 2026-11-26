//! "If your life total would be reduced to 0 or less, instead ..." (`ReplaceWhen::R1193LifeReducedTo`, CR 119.3,
//! 614.1). The losses of life and the damage to a player in one batch happen at once, so they are one reduction judged
//! by the total they leave (combat damage from several creatures included). When the effect applies, none of them takes
//! life: a loss of life does not happen and damage is still dealt without a loss (lifelink and damage triggers read it),
//! and the effects happen in their place, read against the total before the batch ("your life total becomes 3").

use mtg_cards::ability::Ability;
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen};
use mtg_core::ids::PlayerId;

use super::{Candidate, Item, Origin, Pass, Slot, Unit};
use crate::combat::{DamageForm, DamageTarget};
use crate::events::Event;

/// The change in `player`'s life total an event makes: a gain, a loss, a payment or damage that takes life.
fn life_change(item: &Item, player: PlayerId) -> i64 {
    match item {
        Item::One(
            Event::LifeLost {
                player: who,
                amount,
            }
            | Event::LifePaid {
                player: who,
                amount,
            },
        ) if *who == player => -i64::from(*amount),
        Item::One(Event::LifeGained {
            player: who,
            amount,
        }) if *who == player => i64::from(*amount),
        Item::One(Event::DamageDealt {
            target: DamageTarget::Player(who),
            amount,
            form,
            ..
        }) if *who == player => -i64::from(form.life_lost(*amount)),
        _ => 0,
    }
}

impl Pass<'_> {
    /// Whether a reduction to a total covers the event at `at`: the batch leaves the player's life total at or below it.
    /// Any other replacement passes.
    pub(super) fn r1193_reaches(&self, at: usize, when: &ReplaceWhen, unit: &Unit) -> bool {
        let ReplaceWhen::R1193LifeReducedTo { at_most, .. } = when else {
            return true;
        };
        let (Unit::LifeLoss { player }
        | Unit::Damage {
            target: DamageTarget::Player(player),
            ..
        }) = unit
        else {
            return false;
        };
        let Some(life) = self.state.player(*player).map(crate::player::Player::life) else {
            return false;
        };
        let left = self
            .slots
            .iter()
            .map(|slot| life_change(&slot.item, *player))
            .fold(i64::from(life), i64::saturating_add);
        life_change(&self.slots[at].item, *player) < 0 && left <= i64::from(*at_most)
    }

    /// Whether the candidate is a replacement of a reduction of a life total.
    pub(super) fn r1193_is_life_reduction(&self, candidate: &Candidate) -> bool {
        self.sources.iter().any(|source| {
            let origin = if source.copied {
                Origin::Copied(source.address.ability)
            } else {
                Origin::Ability(source.address)
            };
            origin == candidate.origin
                && matches!(
                    &*source.ability,
                    Ability::Replacement {
                        when: ReplaceWhen::R1193LifeReducedTo { .. },
                        by: ReplaceBy::Instead(_),
                        ..
                    }
                )
        })
    }

    /// Takes the life out of every other event of the batch that reduces `player`'s life total: a loss is dropped and
    /// damage is dealt without a loss. Damage at `at`, which the effects take the place of, is dealt again without a
    /// loss just after it. Returns where the event at `at` is now.
    pub(super) fn r1193_absorb(
        &mut self,
        at: usize,
        player: PlayerId,
        applied: &[Origin],
    ) -> usize {
        let mut now = at;
        let mut out = Vec::with_capacity(self.slots.len() + 1);
        for (index, mut slot) in std::mem::take(&mut self.slots).into_iter().enumerate() {
            let reduces = life_change(&slot.item, player) < 0;
            if index == at {
                now = out.len();
                let dealt = without_loss(&slot.item);
                out.push(slot);
                if let Some(item) = dealt {
                    out.push(Slot {
                        item,
                        applied: applied.to_vec(),
                        frozen: true,
                    });
                }
                continue;
            }
            if reduces && matches!(slot.item, Item::One(Event::LifeLost { .. })) {
                continue;
            }
            if reduces && let Some(item) = without_loss(&slot.item) {
                slot.item = item;
            }
            out.push(slot);
        }
        self.slots = out;
        now
    }

    /// Drops the damage dealt again after `at` when the effects could not run and the event at `at` stayed as it was.
    pub(super) fn r1193_kept(&mut self, at: usize) {
        let stayed = matches!(
            self.slots.get(at),
            Some(Slot {
                item: Item::One(Event::DamageDealt { form, .. }),
                ..
            }) if *form != DamageForm::NoLifeLoss
        );
        if stayed && at + 1 < self.slots.len() {
            self.slots.remove(at + 1);
        }
    }
}

/// The damage event dealt without a loss of life; none for any other event.
fn without_loss(item: &Item) -> Option<Item> {
    match item {
        Item::One(Event::DamageDealt {
            source,
            target,
            amount,
            deathtouch,
            toxic,
            unpreventable,
            ..
        }) => Some(Item::One(Event::DamageDealt {
            source: *source,
            target: *target,
            amount: *amount,
            deathtouch: *deathtouch,
            form: DamageForm::NoLifeLoss,
            toxic: *toxic,
            unpreventable: *unpreventable,
        })),
        _ => None,
    }
}
