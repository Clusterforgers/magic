//! A permanent that enters tapped unless its controller reveals a card from their hand ("As this land enters, you may
//! reveal an Elemental card from your hand. If you don't, this land enters tapped.").

use mtg_cards::refs::{Pred, Selector, ZoneKind};
use mtg_cards::value::CounterKind;
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
    /// The cards of `controller`'s hand that `of` names; to behold, also the permanents they control that it names
    /// (a beheld permanent is chosen, a beheld card is revealed).
    fn revealable(
        &self,
        candidate: &Candidate,
        (of, behold): (&Selector, bool),
        controller: PlayerId,
    ) -> Result<Vec<ObjectId>, EngineError> {
        let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
        let mut found: Vec<ObjectId> = Vec::new();
        let mut selectors = vec![of.clone()];
        if behold {
            let mut in_hand = of.clone();
            in_hand.0.push(Pred::In(ZoneKind::Hand));
            selectors = vec![of.clone(), in_hand];
        }
        for selector in &selectors {
            for named in scope.matching(selector)? {
                let Some(card) = self.state.object(named.id) else {
                    continue;
                };
                let held = card.zone() == Zone::Hand(controller);
                let on_battlefield = behold
                    && card.zone() == Zone::Battlefield
                    && card.controller() == controller
                    && named.id != candidate.object.id;
                if (held || on_battlefield) && !found.contains(&named.id) {
                    found.push(named.id);
                }
            }
        }
        Ok(found)
    }

    /// Whether `controller` reveals a card: asked yes or no, with No the only answer when they hold no such card, so the
    /// pace does not show their hand.
    fn reveals(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        (possible, behold): (bool, bool),
    ) -> Result<Asked<bool>, EngineError> {
        let question = if behold {
            "Behold a permanent you control or a card from your hand?"
        } else {
            "Reveal a card from your hand?"
        };
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions {
                    only_no: !possible,
                    unseen: true,
                    ..YesNoOptions::new(source, question.to_owned())
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
                    one_per_name: false,
                    source,
                    from: options.to_vec(),
                    min: 1,
                    max: 1,
                    text: "Choose the card to reveal.".into(),
                    can_cancel: false,
                    total: None,
                    groups: Vec::new(),
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

    /// The permanent `object` enters tapped unless `controller` reveals a card of their hand that `of` names (or, to
    /// behold, chooses a permanent they control that it names); the reveal is an event that every seat sees, placed
    /// before the entering, and a beheld permanent is chosen without one.
    pub(super) fn reveal_or_tapped(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (of, behold): (&Selector, bool),
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let source = candidate.object.id;
        let options = self.revealable(candidate, (of, behold), controller)?;
        let reveal = match self.reveals(source, controller, (!options.is_empty(), behold))? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(reveal) => reveal,
        };
        if reveal {
            let card = match self.revealed_card(source, controller, &options)? {
                Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                Asked::Got(card) => card,
            };
            let in_hand = self
                .state
                .object(card)
                .is_some_and(|it| it.zone() == Zone::Hand(controller));
            if !in_hand {
                // A beheld permanent is only chosen: the land enters untapped and nothing is revealed.
                self.slots[at].applied = applied;
                return Ok(Applied::Done);
            }
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

    /// The permanent `object` enters with `count` counters when `controller` controls another permanent `of` names, or
    /// else reveals a card of their hand it names (asked only then); the reveal is placed before the entering.
    pub(super) fn r6007_reveal_or_control(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (of, counter, count): (&Selector, &CounterKind, u32),
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
        let controls = scope.matching(of)?.into_iter().any(|named| {
            named.id != object
                && self.state.object(named.id).is_some_and(|permanent| {
                    permanent.zone() == Zone::Battlefield
                        && crate::characteristics::controller_of(self.state, named.id)
                            == Some(controller)
                })
        });
        let mut entering = at;
        if !controls {
            let mut in_hand = of.clone();
            in_hand.0.push(Pred::In(ZoneKind::Hand));
            let source = candidate.object.id;
            let options = self.revealable(candidate, (&in_hand, false), controller)?;
            let reveal = match self.reveals(source, controller, (!options.is_empty(), false))? {
                Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                Asked::Got(reveal) => reveal,
            };
            if !reveal {
                if let Some(slot) = self.slots.get_mut(at) {
                    slot.applied = applied;
                }
                return Ok(Applied::Done);
            }
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
            entering = at + 1;
        }
        let count = mtg_cards::value::Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX));
        self.enter_with_counters(entering, candidate, object, counter, &count, &applied)?;
        self.slots[entering].applied = applied;
        Ok(Applied::Done)
    }
}
