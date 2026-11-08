//! What a permanent has besides its counters when it enters ("enters with two +1/+1 counters on it and with first strike",
//! CR 614.1c): keywords and abilities it keeps while it stays on the battlefield, made with the events that bring it in.

use mtg_cards::modify::{Duration, Modification};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Item, Origin, Pass, Slot};
use crate::effects::{ContinuousEffect, Modification as Change, Selector as Affected};
use crate::error::EngineError;
use crate::events::Event;
use crate::object::ObjectRef;

impl Pass<'_> {
    /// Gives the permanent the card `object` becomes each keyword or ability of `mods`; any other change is not one a
    /// permanent can enter with.
    pub(super) fn enter_with_changes(
        &mut self,
        at: usize,
        object: ObjectId,
        controller: PlayerId,
        mods: &[Modification],
        applied: &[Origin],
    ) -> Result<(), EngineError> {
        let Some(card) = self.state.object(object) else {
            return Ok(());
        };
        // The changes are for the permanent the card becomes (CR 400.7).
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let mut inherited = applied.to_vec();
        inherited.dedup();
        for modification in mods {
            let change = match modification {
                Modification::AddKeyword(keyword) => Change::AddKeyword(keyword.clone()),
                Modification::GrantAbility(ability) => {
                    Change::GrantAbility(std::sync::Arc::clone(ability))
                }
                other => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no change a permanent enters with"
                    )));
                }
            };
            let (id, timestamp) = self.fresh_effect();
            let effect = Event::EffectCreated {
                effect: Box::new(
                    ContinuousEffect::new(
                        id,
                        Affected::Objects(vec![permanent]),
                        change,
                        Duration::WhileSourceOnBattlefield,
                        timestamp,
                    )
                    .from_source(permanent)
                    .begun(controller, self.state.turn().number()),
                ),
            };
            // Effects of one batch stay in the order of their ids.
            let place = self
                .slots
                .iter()
                .rposition(|slot| matches!(&slot.item, Item::One(Event::EffectCreated { .. })))
                .map_or(at + 1, |last| (last + 1).max(at + 1));
            self.slots.insert(
                place,
                Slot {
                    item: Item::One(effect),
                    applied: inherited.clone(),
                    frozen: false,
                },
            );
        }
        Ok(())
    }
}
