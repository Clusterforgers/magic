//! "If an opponent would draw two or more cards, instead ..." (`ReplaceWhen::R6603DrawsSeveral`): the draws of one
//! player that one event of drawing proposes are a batch of `CardDrawn` (or `DrewFromEmptyLibrary`) events, one per
//! card; the replacement matches while that player still has at least so many draws in the batch, and applying it
//! takes every one of them away and puts what happens instead in place of the first, so the whole draw is replaced at
//! once and the draws it makes (one card each) are new events (CR 614.5, Alms Collector rulings).

use mtg_core::ids::PlayerId;

use super::{Candidate, Item, Origin, Pass};
use crate::events::Event;
use mtg_cards::ability::Ability;
use mtg_cards::trigger::ReplaceWhen;

impl Pass<'_> {
    /// The places of the batch's draws of `player` that no replacement has taken yet.
    pub(super) fn r6603_draws_of(&self, player: PlayerId) -> Vec<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                !slot.frozen
                    && matches!(
                        &slot.item,
                        Item::One(
                            Event::CardDrawn { player: drawer, .. }
                                | Event::DrewFromEmptyLibrary { player: drawer }
                        ) if *drawer == player
                    )
            })
            .map(|(at, _)| at)
            .collect()
    }

    /// Whether the candidate is a replacement of a draw of several cards.
    pub(super) fn r6603_several(&self, candidate: &Candidate) -> bool {
        let Origin::Ability(address) = candidate.origin else {
            return false;
        };
        self.sources.iter().any(|source| {
            source.address == address
                && matches!(
                    &*source.ability,
                    Ability::Replacement {
                        when: ReplaceWhen::R6603DrawsSeveral { .. },
                        ..
                    }
                )
        })
    }

    /// Takes away every draw of `player` in the batch but the one at `at` and gives the new place of that one.
    pub(super) fn r6603_one_draw(&mut self, at: usize, player: PlayerId) -> usize {
        let others: Vec<usize> = self
            .r6603_draws_of(player)
            .into_iter()
            .filter(|place| *place != at)
            .collect();
        let before = others.iter().filter(|place| **place < at).count();
        for place in others.into_iter().rev() {
            self.slots.remove(place);
        }
        self.draws_changed = true;
        at - before
    }
}
