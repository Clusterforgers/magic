//! An object chosen as an attachment becomes attached (`ReplaceWhen::BecomesAttached` with `ReplaceBy::R114ChooseObject`,
//! CR 614.1c): "As this Equipment becomes attached to a creature, choose an exiled creature card used to craft this
//! Equipment." The attachment remembers the object chosen for `Ref::R114ChosenObject`, in place of any earlier one, so
//! its abilities read the last chosen object; the object may be in any zone the selector names.

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::memory::Remembered;
use crate::resolve::Scope;

impl Pass<'_> {
    /// The controller of `candidate` chooses one object `what` names (asked when there are several), which the attachment
    /// `object` remembers before its attaching at `at`; nothing is chosen, and the earlier choice stays, with none.
    pub(super) fn r121_choose_object_as_attached(
        &mut self,
        at: usize,
        candidate: &Candidate,
        what: &Selector,
        object: ObjectId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let controller = candidate.controller;
        let scope = Scope::announcing(self.state, controller, candidate.object);
        let mut options: Vec<ObjectId> = scope
            .matching(what)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| *id != object)
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
                            text: "Choose a card.".into(),
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
        self.slots[at].applied = applied;
        let (Some(chosen), Some(attachment)) = (
            chosen.and_then(|id| self.state.object(id)),
            self.state.object(object),
        ) else {
            return Ok(Applied::Done);
        };
        let remembered = Remembered {
            source: attachment.reference(),
            slot: super::r114_choose_object::r114_chosen_slot(),
            objects: vec![chosen.reference()],
            values: Vec::new(),
            replace: true,
        };
        self.slots.insert(
            at,
            Slot {
                item: Item::One(Event::Remembered(Box::new(remembered))),
                applied: Vec::new(),
                frozen: true,
            },
        );
        Ok(Applied::Done)
    }
}
