//! A loss of life made a multiple of itself (`ReplaceBy::LossTimes`, CR 614.1): "If an opponent would lose life during
//! your turn, they lose twice that much life instead." A loss by an effect is a `LifeLost` of the larger amount; damage
//! to a player is dealt as it was and only the life it takes grows (`DamageForm::LossTimes`), so lifelink and damage
//! triggers read the damage dealt.

use super::{Item, Origin, Pass, Slot};
use crate::combat::DamageForm;
use crate::events::Event;

impl Pass<'_> {
    /// The event at `at` with the life it takes `times` as much.
    pub(super) fn r4703_loss_times(&mut self, at: usize, times: u32, applied: &[Origin]) {
        let item = match self.slots[at].item.clone() {
            Item::One(Event::LifeLost { player, amount }) => Item::One(Event::LifeLost {
                player,
                amount: amount.saturating_mul(times),
            }),
            Item::One(Event::DamageDealt {
                source,
                target,
                amount,
                deathtouch,
                form,
                toxic,
                unpreventable,
            }) => {
                let form = match form {
                    DamageForm::LossTimes(already) => {
                        DamageForm::LossTimes(already.saturating_mul(times))
                    }
                    _ => DamageForm::LossTimes(times),
                };
                Item::One(Event::DamageDealt {
                    source,
                    target,
                    amount,
                    deathtouch,
                    form,
                    toxic,
                    unpreventable,
                })
            }
            other => other,
        };
        self.slots[at] = Slot {
            item,
            applied: applied.to_vec(),
            frozen: false,
        };
    }
}
