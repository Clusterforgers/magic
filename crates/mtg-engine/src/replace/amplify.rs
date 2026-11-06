//! Amplify N (CR 702.38) in the replacement step: as the creature enters, its controller reveals any number of creature
//! cards from their hand that share a creature type with it, and it enters with N +1/+1 counters for each.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::characteristics::{characteristics, creature_types};
use crate::choose::{CardsOptions, Choice};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::library::Revealed;
use crate::object::ObjectRef;
use crate::zones::Zone;
use mtg_cards::value::CounterKind;

impl Pass<'_> {
    /// The creature cards of `controller`'s hand that share a creature type with the card `object`.
    fn amplifiers(&self, object: ObjectId, controller: PlayerId) -> Vec<ObjectId> {
        let (types, every) = creature_types(self.state, object);
        self.state
            .zone(Zone::Hand(controller))
            .iter()
            .copied()
            .filter(|id| *id != object)
            .filter(|id| characteristics(self.state, *id).is_some_and(|chars| chars.is_creature()))
            .filter(|id| {
                let (theirs, all) = creature_types(self.state, *id);
                (every && (all || !theirs.is_empty()))
                    || (all && !types.is_empty())
                    || theirs.iter().any(|kind| types.contains(kind))
            })
            .collect()
    }

    /// Reveals the cards `controller` picks and puts `per` counters on the permanent for each. Nobody is asked when no card
    /// of the hand could be revealed, so the pace does not show the hand; the reveal is an event every seat sees.
    pub(super) fn amplify(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (object, controller): (ObjectId, PlayerId),
        per: u32,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let options = self.amplifiers(object, controller);
        let chosen: Vec<ObjectId> = if options.is_empty() {
            Vec::new()
        } else {
            match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseCards(CardsOptions {
                            source: candidate.object.id,
                            max: u32::try_from(options.len()).unwrap_or(u32::MAX),
                            from: options,
                            min: 0,
                            text: "Choose creature cards to reveal.".into(),
                            can_cancel: false,
                        }),
                    }));
                }
                Some((who, Choice::Cards(cards)))
                    if *who == controller
                        && cards.iter().all(|card| options.contains(card))
                        && cards
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            == cards.len() =>
                {
                    cards.clone()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the amplify reveal of {controller:?}"
                    )));
                }
            }
        };
        let Some(card) = self.state.object(object) else {
            return Ok(Applied::Done);
        };
        let delta = u32::try_from(chosen.len())
            .ok()
            .and_then(|made| made.checked_mul(per))
            .and_then(|total| i32::try_from(total).ok())
            .filter(|total| *total > 0);
        let Some(delta) = delta else {
            self.slots[at].applied = applied;
            return Ok(Applied::Done);
        };
        // The counters go on the permanent the card becomes (CR 400.7).
        let on = crate::choose::Target::Object(ObjectRef {
            id: object,
            counter: card.counter() + 1,
        });
        let mut inherited = applied.clone();
        inherited.dedup();
        self.slots.insert(
            at,
            Slot {
                item: Item::One(Event::Revealed(Revealed {
                    player: controller,
                    cards: chosen,
                })),
                applied: Vec::new(),
                frozen: true,
            },
        );
        // The enter event moved one place on.
        self.slots[at + 1].applied = applied;
        self.slots.insert(
            at + 2,
            Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on,
                    counter: CounterKind::PlusOnePlusOne,
                    delta,
                })),
                applied: inherited,
                frozen: false,
            },
        );
        Ok(Applied::Done)
    }
}
