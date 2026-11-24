//! An object chosen as a permanent enters (`ReplaceBy::R114ChooseObject`, CR 614.1c): "As this Aura enters, choose a
//! creature." The permanent the card becomes (CR 400.7) remembers the object chosen, as the object it was then, for
//! `Ref::R114ChosenObject`; only an object already on the battlefield can be chosen.

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::memory::Remembered;
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::zones::Zone;

/// The memory slot of the object chosen as the permanent entered: written on the permanent itself, so it is not one
/// of the lasting slots an entering card carries along, and no card file names it.
pub(crate) fn r114_chosen_slot() -> String {
    "~r114 chosen object".to_owned()
}

impl Pass<'_> {
    /// The entering `object` under `controller` chooses one permanent `what` names on the battlefield (asked when there
    /// are several), which the permanent remembers from the moment it is on the battlefield; nothing with none.
    pub(super) fn r114_choose_object(
        &mut self,
        at: usize,
        candidate: &Candidate,
        what: &Selector,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let scope = Scope::announcing(self.state, controller, candidate.object);
        let mut options: Vec<ObjectId> = scope
            .matching(what)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                *id != object
                    && self
                        .state
                        .object(*id)
                        .is_some_and(|other| other.zone() == Zone::Battlefield)
            })
            .collect();
        options.sort_unstable();
        options.dedup();
        let chosen = match options.as_slice() {
            [] => None,
            [one] => Some(*one),
            _ => match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseCards(CardsOptions {
                            one_per_name: false,
                            source: candidate.object.id,
                            from: options,
                            min: 1,
                            max: 1,
                            text: "Choose a creature.".into(),
                            can_cancel: false,
                            total: None,
                            groups: Vec::new(),
                            targeted: Box::default(),
                        }),
                    }));
                }
                Some((who, Choice::Cards(cards)))
                    if *who == controller
                        && cards.len() == 1
                        && cards.iter().all(|card| options.contains(card)) =>
                {
                    cards.first().copied()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the object {controller:?} chooses for {object:?}"
                    )));
                }
            },
        };
        self.slots[at].applied.clone_from(&applied);
        let (Some(chosen), Some(card)) = (
            chosen.and_then(|id| self.state.object(id)),
            self.state.object(object),
        ) else {
            return Ok(Applied::Done);
        };
        let permanent = ObjectRef {
            id: object,
            counter: card.counter() + 1,
        };
        let remembered = Remembered {
            source: permanent,
            slot: r114_chosen_slot(),
            objects: vec![chosen.reference()],
            values: Vec::new(),
            replace: true,
        };
        let mut inherited = applied;
        inherited.dedup();
        let entered = self.entered_at(at, object, controller);
        self.slots.insert(
            entered + 1,
            Slot {
                item: Item::One(Event::Remembered(Box::new(remembered))),
                applied: inherited,
                frozen: false,
            },
        );
        Ok(Applied::Done)
    }
}
