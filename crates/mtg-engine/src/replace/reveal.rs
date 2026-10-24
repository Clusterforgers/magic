//! A permanent that enters tapped unless its controller reveals a card from their hand ("As this land enters, you may
//! reveal an Elemental card from your hand. If you don't, this land enters tapped.").

use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Asked, Candidate, Item, Origin, Pass, Slot, entering};
use crate::choose::{CardsOptions, Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::library::Revealed;
use crate::resolve::Scope;
use crate::zones::Zone;

impl Pass<'_> {
    /// The cards of `controller`'s hand that `of` names.
    fn revealable(
        &self,
        candidate: &Candidate,
        of: &Selector,
        controller: PlayerId,
    ) -> Result<Vec<ObjectId>, EngineError> {
        let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
        Ok(scope
            .matching(of)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                self.state
                    .object(*id)
                    .is_some_and(|card| card.zone() == Zone::Hand(controller))
            })
            .collect())
    }

    /// Whether `controller` reveals a card: asked yes or no, with No the only answer when they hold no such card, so the
    /// pace does not show their hand.
    fn reveals(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        possible: bool,
    ) -> Result<Asked<bool>, EngineError> {
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions {
                    only_no: !possible,
                    ..YesNoOptions::new(source, "Reveal a card from your hand?".to_owned())
                }),
            })),
            Some((who, Choice::YesNo(yes))) if *who == controller && (!*yes || possible) => {
                Ok(Asked::Got(*yes))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the reveal of {controller:?}"
            ))),
        }
    }

    /// The card `controller` reveals, asked unless one card is revealable.
    fn revealed_card(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        options: &[ObjectId],
    ) -> Result<Asked<ObjectId>, EngineError> {
        if let [only] = options {
            return Ok(Asked::Got(*only));
        }
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::ChooseCards(CardsOptions {
                    source,
                    from: options.to_vec(),
                    min: 1,
                    max: 1,
                    text: "Choose the card to reveal.".into(),
                    can_cancel: false,
                }),
            })),
            Some((who, Choice::Cards(cards)))
                if *who == controller
                    && cards.len() == 1
                    && cards.iter().all(|card| options.contains(card)) =>
            {
                Ok(Asked::Got(cards[0]))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the choice of the card {controller:?} reveals"
            ))),
        }
    }

    /// The permanent `object` enters tapped unless `controller` reveals a card of their hand that `of` names; the reveal
    /// is an event that every seat sees, placed before the entering.
    pub(super) fn reveal_or_tapped(
        &mut self,
        at: usize,
        candidate: &Candidate,
        of: &Selector,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let source = candidate.object.id;
        let options = self.revealable(candidate, of, controller)?;
        let reveal = match self.reveals(source, controller, !options.is_empty())? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(reveal) => reveal,
        };
        if reveal {
            let card = match self.revealed_card(source, controller, &options)? {
                Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                Asked::Got(card) => card,
            };
            self.slots.insert(
                at,
                Slot {
                    item: Item::One(Event::Revealed(Revealed {
                        player: controller,
                        cards: vec![card],
                    })),
                    applied: Vec::new(),
                    frozen: true,
                },
            );
            // The enter event moved one place on.
            self.slots[at + 1].applied = applied;
            return Ok(Applied::Done);
        }
        let entered = self.entered_at(at, object, controller);
        if let Item::One(Event::Entered { tapped, .. }) = &mut self.slots[entered].item {
            *tapped = true;
        }
        if let Some(slot) = self.slots.get(at)
            && matches!(&slot.item, Item::One(event) if entering(self.state, event).is_some())
        {
            self.slots[at].applied = applied;
        }
        Ok(Applied::Done)
    }
}
