//! `ReplaceBy::R7071ExileTwoAsCopy` (CR 614.1c, 707.9): as the permanent enters, its controller may exile two cards the
//! selector names; if they do, it enters as a copy of the one they choose, with as many additional +1/+1 counters as
//! the other card's power (The Mimeoplasm).

use mtg_cards::refs::Selector;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Candidate, Item, Origin, Pass, Slot};
use crate::choose::{CardsOptions, Choice, Target};
use crate::copies::{copiable_values, may_be_copied};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectRef;
use crate::resolve::Scope;
use crate::zones::Zone;

/// A choice of cards from `from`, between `min` and `max` of them.
fn ask(
    player: PlayerId,
    source: ObjectId,
    from: Vec<ObjectId>,
    (min, max): (u32, u32),
    text: &str,
) -> Decision {
    Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
            source,
            from,
            min,
            max,
            text: text.into(),
            can_cancel: false,
            total: None,
            groups: Vec::new(),
            targeted: Box::default(),
        }),
    }
}

impl Pass<'_> {
    /// The entering goes on as it is, with the effect marked as applied to it.
    fn r7071_unchanged(&mut self, at: usize, applied: Vec<Origin>) -> Applied {
        if let Some(slot) = self.slots.get_mut(at) {
            slot.applied = applied;
        }
        Applied::Done
    }

    /// Asks for the two cards (or none), then for the one copied; puts the exiles before the entering, the copy effect
    /// and the counters after it. Nobody is asked when fewer than two cards can be exiled.
    pub(super) fn r7071_exile_two(
        &mut self,
        at: usize,
        candidate: &Candidate,
        of: &Selector,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let scope = Scope::announcing(self.state, controller, candidate.object);
        let options: Vec<ObjectId> = scope
            .matching(of)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                *id != object
                    && self.state.object(*id).is_some_and(|card| {
                        matches!(card.zone(), Zone::Graveyard(_)) && may_be_copied(card)
                    })
            })
            .collect();
        if options.len() < 2 {
            return Ok(self.r7071_unchanged(at, applied));
        }
        let source = candidate.object.id;
        let exiled = match self.answer() {
            None => {
                return Ok(Applied::Ask(ask(
                    controller,
                    source,
                    options,
                    (0, 2),
                    "Choose two creature cards to exile, or none.",
                )));
            }
            Some((who, Choice::Cards(cards)))
                if *who == controller
                    && (cards.is_empty() || (cards.len() == 2 && cards[0] != cards[1]))
                    && cards.iter().all(|card| options.contains(card)) =>
            {
                cards.clone()
            }
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the cards {controller:?} exiles"
                )));
            }
        };
        let [first, second] = exiled.as_slice() else {
            return Ok(self.r7071_unchanged(at, applied));
        };
        let copied = match self.answer() {
            None => {
                return Ok(Applied::Ask(ask(
                    controller,
                    source,
                    vec![*first, *second],
                    (1, 1),
                    "Choose the card to copy.",
                )));
            }
            Some((who, Choice::Cards(cards)))
                if *who == controller && cards.len() == 1 && exiled.contains(&cards[0]) =>
            {
                cards[0]
            }
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the card {controller:?} copies"
                )));
            }
        };
        let other = if copied == *first { *second } else { *first };
        let reference = |id: ObjectId| {
            self.state
                .object(id)
                .map(crate::object::GameObject::reference)
        };
        let (Some(copied_ref), Some(other_ref)) = (reference(copied), reference(other)) else {
            return Ok(self.r7071_unchanged(at, applied));
        };
        let values = copiable_values(self.state, copied_ref, None)
            .ok_or_else(|| Self::invalid(format!("{copied_ref:?} cannot be copied")))?;
        let power = crate::characteristics::characteristics(self.state, other_ref.id)
            .and_then(|chars| chars.power)
            .unwrap_or(0);
        let copy = Self::r109_copy_of(&scope, object, (copied_ref, values), (&[], &[]), None)?;
        let exiles: Vec<Slot> = exiled
            .iter()
            .filter_map(|id| {
                let from = self.state.object(*id)?.zone();
                Some(Slot::new(Item::One(Event::ZoneChanged {
                    object: *id,
                    from,
                    to: Zone::Exile,
                    cause: ZoneCause::Exiled,
                })))
            })
            .collect();
        let made = exiles.len();
        self.slots.splice(at..at, exiles);
        let entering = at + made;
        if let Some(slot) = self.slots.get_mut(entering) {
            slot.applied.clone_from(&applied);
        }
        self.begin_copy(entering, copy, &applied);
        if power > 0
            && let Some(card) = self.state.object(object)
        {
            let mut inherited = applied;
            inherited.dedup();
            // The counters go on the permanent the card becomes (CR 400.7).
            let on = Target::Object(ObjectRef {
                id: object,
                counter: card.counter() + 1,
            });
            self.slots.insert(
                entering + 2,
                Slot {
                    item: Item::One(Event::CountersChanged(CountersChanged {
                        on,
                        counter: CounterKind::PlusOnePlusOne,
                        delta: power,
                    })),
                    applied: inherited,
                    frozen: false,
                },
            );
        }
        Ok(Applied::Done)
    }
}
