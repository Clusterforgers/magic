//! Dredge (CR 702.52a): a draw that a card in the graveyard replaces. The player mills cards from the top of the library
//! and returns the dredging card to their hand; no card is drawn, so nothing that watches draws sees one.

use mtg_cards::ability::Ability;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Candidate, Item, Pass, Slot, Source, moving};
use crate::ability_reader::printed_and_provided;
use crate::characteristics::controller_of_object;
use crate::choose::{Choice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::object::GameObject;
use crate::zones::Zone;

impl Pass<'_> {
    /// The cards in the graveyard of `player` as the batch has left them before the slot at `at`.
    fn graveyard_at(&self, at: usize, player: PlayerId) -> Vec<ObjectId> {
        let graveyard = Zone::Graveyard(player);
        let mut cards = self.state.zone(graveyard).to_vec();
        for slot in &self.slots[..at] {
            let Item::One(event) = &slot.item else {
                continue;
            };
            let Some((object, from, to)) = moving(self.state, event) else {
                continue;
            };
            if from == graveyard {
                cards.retain(|card| *card != object);
            }
            if to == graveyard {
                cards.push(object);
            }
        }
        cards
    }

    /// The library of `player`, top card first, as the slot at `at` finds it: the draws of the batch before it have taken
    /// their cards and the cards that left the library in place of being drawn are gone.
    pub(super) fn library_at(&self, at: usize, player: PlayerId) -> Vec<ObjectId> {
        let before = self.slots[..at]
            .iter()
            .filter(|slot| {
                matches!(
                    &slot.item,
                    Item::One(Event::CardDrawn { player: who, .. } | Event::DrewFromEmptyLibrary { player: who })
                        if *who == player
                )
            })
            .count();
        self.state
            .zone(Zone::Library(player))
            .iter()
            .rev()
            .filter(|card| !self.pulled.contains(card))
            .skip(before)
            .copied()
            .collect()
    }

    /// Whether the card `card` can replace the draw at `at`: it lies in the graveyard of the player who draws, which holds
    /// `cards` cards to mill (CR 702.52b).
    pub(super) fn can_dredge(&self, at: usize, card: ObjectId, cards: u32) -> bool {
        let Item::One(Event::CardDrawn { player, .. }) = &self.slots[at].item else {
            return false;
        };
        let wanted = usize::try_from(cards).unwrap_or(usize::MAX);
        self.graveyard_at(at, *player).contains(&card)
            && self.library_at(at, *player).len() >= wanted
    }

    /// Whether the player dredges instead of drawing: asked yes or no. A no is remembered for the draw, which then goes on
    /// with the other effects.
    pub(super) fn dredge_wanted(
        &mut self,
        at: usize,
        candidate: &Candidate,
        cards: u32,
    ) -> Result<Asked<bool>, EngineError> {
        let player = candidate.controller;
        match self.answer() {
            None => {
                let name = self
                    .state
                    .object(candidate.object.id)
                    .map_or("the card", |card| card.card().name.as_str());
                Ok(Asked::Ask(Decision {
                    player,
                    kind: DecisionKind::YesNo(YesNoOptions::new(
                        candidate.object.id,
                        format!(
                            "Mill {cards} cards and return {name} to your hand instead of drawing?"
                        ),
                    )),
                }))
            }
            Some((who, Choice::YesNo(yes))) if *who == player => {
                let yes = *yes;
                if !yes {
                    self.slots[at].applied.push(candidate.origin);
                }
                Ok(Asked::Got(yes))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the dredge question of {player:?}"
            ))),
        }
    }

    /// Replaces the draw at `at` by the mill of `cards` cards and the return of the dredging card to its owner's hand. The
    /// milled cards are out of the library for the draws after it, and a milled card that dredges can replace one of them.
    pub(super) fn dredge(&mut self, at: usize, candidate: &Candidate, cards: u32) {
        let Item::One(Event::CardDrawn { player, .. }) = self.slots[at].item.clone() else {
            return;
        };
        let Some(owner) = self
            .state
            .object(candidate.object.id)
            .map(GameObject::owner)
        else {
            return;
        };
        let wanted = usize::try_from(cards).unwrap_or(usize::MAX);
        let milled: Vec<ObjectId> = self
            .library_at(at, player)
            .into_iter()
            .take(wanted)
            .collect();
        let returned = Event::ZoneChanged {
            object: candidate.object.id,
            from: Zone::Graveyard(owner),
            to: Zone::Hand(owner),
            cause: ZoneCause::Moved,
        };
        let moves: Vec<Slot> = milled
            .iter()
            .map(|&object| Event::ZoneChanged {
                object,
                from: Zone::Library(player),
                to: Zone::Graveyard(player),
                cause: ZoneCause::Milled,
            })
            .chain(std::iter::once(returned))
            .map(|event| Slot::new(Item::One(event)))
            .collect();
        self.slots.splice(at..=at, moves);
        for &card in &milled {
            self.add_graveyard_sources(card);
        }
        self.pulled.extend(milled);
        self.draws_changed = true;
    }

    /// The replacement effects of a card that is milled that work from a graveyard, which the later draws of the batch
    /// find there.
    fn add_graveyard_sources(&mut self, card: ObjectId) {
        let Some(object) = self.state.object(card) else {
            return;
        };
        let controller = controller_of_object(self.state, object);
        let found: Vec<Source> = printed_and_provided(object)
            .into_iter()
            .filter(|(_, rules)| {
                matches!(&**rules, Ability::Replacement { from, .. } if *from == ZoneKind::Graveyard)
            })
            .map(|(address, ability)| Source {
                address,
                ability,
                controller,
                object: object.reference(),
                copied: false,
            })
            .collect();
        self.sources.extend(found);
    }
}
