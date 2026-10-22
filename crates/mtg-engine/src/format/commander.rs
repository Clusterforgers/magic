//! The Commander format (CR 903): a 100-card singleton deck led by one commander or a pair of partners, played from a
//! command zone, with 40 life, a free first mulligan, a tax on casting the commander and commander damage.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::PlayerId;

use super::{DeckError, ExtraZone, FormatRules, MulliganRule};
use crate::commanders::damage_dealt_to;
use crate::player::LossReason;
use crate::state::GameState;

/// Cards in a Commander deck, its commanders included.
pub const COMMANDER_DECK: usize = 100;
/// Life each player starts with in Commander.
pub const COMMANDER_LIFE: i32 = 40;
/// Combat damage from one commander that makes a player lose.
pub const COMMANDER_DAMAGE: u32 = 21;
/// Generic mana a commander costs more for each cast from the command zone.
pub const COMMANDER_TAX: u32 = 2;

/// The Commander format.
#[derive(Debug, Clone, Copy, Default)]
pub struct Commander;

/// The letters of the colors in `set`, in WUBRG order; `C` for none.
fn letters(set: ColorSet) -> String {
    let word: String = Color::ALL
        .iter()
        .filter(|color| set.contains(**color))
        .map(|color| match color {
            Color::White => 'W',
            Color::Blue => 'U',
            Color::Black => 'B',
            Color::Red => 'R',
            Color::Green => 'G',
        })
        .collect();
    if word.is_empty() { "C".into() } else { word }
}

/// The commanders are one legendary creature (or a card that can be) or two that are partners.
fn check_commanders(commanders: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
    match commanders {
        [] => return Err(DeckError::NoCommander),
        [_] => {}
        [first, second] => {
            if !first.partners_with(second) {
                return Err(DeckError::NotPartners {
                    first: first.name.clone(),
                    second: second.name.clone(),
                });
            }
        }
        more => return Err(DeckError::TooManyCommanders { got: more.len() }),
    }
    match commanders.iter().find(|card| !card.can_be_commander()) {
        Some(card) => Err(DeckError::NotACommander {
            name: card.name.clone(),
        }),
        None => Ok(()),
    }
}

impl FormatRules for Commander {
    fn name(&self) -> &'static str {
        "commander"
    }

    fn starting_life(&self) -> i32 {
        COMMANDER_LIFE
    }

    fn validate_deck(&self, deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        self.validate_seat(deck, &[])
    }

    fn validate_seat(
        &self,
        deck: &[Arc<CardDefinition>],
        commanders: &[Arc<CardDefinition>],
    ) -> Result<(), DeckError> {
        check_commanders(commanders)?;
        let size = deck.len() + commanders.len();
        if size != COMMANDER_DECK {
            return Err(DeckError::WrongSize {
                expected: COMMANDER_DECK,
                got: size,
            });
        }
        let mut copies: BTreeMap<&str, (usize, Option<u32>)> = BTreeMap::new();
        for card in commanders.iter().chain(deck) {
            let entry = copies
                .entry(card.name.as_str())
                .or_insert((0, card.singleton_limit()));
            entry.0 += 1;
        }
        let over = copies
            .into_iter()
            .find(|(_, (got, limit))| limit.is_some_and(|max| *got > max as usize));
        if let Some((name, (got, Some(max)))) = over {
            return Err(DeckError::TooManyCopies {
                name: name.to_owned(),
                max: max as usize,
                got,
            });
        }
        let identity = commanders
            .iter()
            .fold(ColorSet::EMPTY, |all, card| {
                card.color_identity().iter().fold(all, ColorSet::with)
            });
        for card in deck {
            let own = card.color_identity();
            if !own.is_subset_of(identity) {
                let outside = own
                    .iter()
                    .filter(|color| !identity.contains(*color))
                    .fold(ColorSet::EMPTY, ColorSet::with);
                return Err(DeckError::OutsideColorIdentity {
                    name: card.name.clone(),
                    outside: letters(outside),
                    identity: letters(identity),
                });
            }
        }
        Ok(())
    }

    fn mulligan_rule(&self) -> MulliganRule {
        // The first mulligan is free (CR 103.5c).
        MulliganRule {
            hand_size: 7,
            free_mulligans: 1,
        }
    }

    fn extra_zones(&self) -> &'static [ExtraZone] {
        &[ExtraZone::Command]
    }

    // Only the first player of a two-player game skips the draw (CR 103.8a, 103.8c).
    fn skips_first_draw(&self, player_count: usize) -> bool {
        player_count == 2
    }

    fn commander_tax(&self) -> u32 {
        COMMANDER_TAX
    }

    fn commander_damage(&self) -> Option<u32> {
        Some(COMMANDER_DAMAGE)
    }

    fn loss_condition(&self, state: &GameState, player: PlayerId) -> Option<LossReason> {
        let threshold = state.ruleset().commander_damage()?;
        (damage_dealt_to(state, player) >= threshold).then_some(LossReason::CommanderDamage)
    }
}
