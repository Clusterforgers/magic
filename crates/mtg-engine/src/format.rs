//! Format rules: the hooks that make Normal, and later Commander, different games on one engine.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use mtg_core::card::{CardDefinition, Supertype};
use mtg_core::ids::PlayerId;
use thiserror::Error;

use crate::player::LossReason;
use crate::state::{GameResult, GameState};

/// Why a deck is not legal in a format.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeckError {
    /// Fewer cards than the format's minimum.
    #[error("deck has {got} cards but needs at least {min}")]
    TooFewCards {
        /// Required minimum.
        min: usize,
        /// Cards in the deck.
        got: usize,
    },
    /// More copies of a card than allowed.
    #[error("{name} appears {got} times; the limit is {max}")]
    TooManyCopies {
        /// Card name.
        name: String,
        /// Allowed copies.
        max: usize,
        /// Copies in the deck.
        got: usize,
    },
}

/// How a format handles the opening-hand mulligan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MulliganRule {
    /// Cards in an opening hand.
    pub hand_size: usize,
    /// Mulligans that do not cost a card at the bottom step.
    pub free_mulligans: u32,
}

impl MulliganRule {
    /// Cards a player who has taken `mulligans` mulligans puts on the bottom (London mulligan).
    pub fn bottom_count(&self, mulligans: u32) -> usize {
        mulligans.saturating_sub(self.free_mulligans) as usize
    }
}

/// A zone a format adds on top of the standard ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraZone {
    /// The command zone (Commander extension point).
    Command,
}

/// Format-specific rules.
///
/// Extension point (M5): `Commander` implements this trait with 100-card singleton validation,
/// [`ExtraZone::Command`], 40 starting life, a free first mulligan and a commander-damage
/// [`loss_condition`](FormatRules::loss_condition); the engine core does not change.
pub trait FormatRules: Debug + Send + Sync {
    /// Short stable name, hashed into the game state.
    fn name(&self) -> &'static str;
    /// Life each player starts with.
    fn starting_life(&self) -> i32;
    /// Checks a player's deck.
    fn validate_deck(&self, deck: &[Arc<CardDefinition>]) -> Result<(), DeckError>;
    /// The opening-hand mulligan rule.
    fn mulligan_rule(&self) -> MulliganRule;
    /// Zones beyond library, hand, battlefield, graveyard, stack and exile.
    fn extra_zones(&self) -> &'static [ExtraZone] {
        &[]
    }
    /// Whether the starting player skips their first draw.
    fn skips_first_draw(&self, player_count: usize) -> bool;
    /// A format-specific reason `player` has lost, checked with state-based actions.
    fn loss_condition(&self, _state: &GameState, _player: PlayerId) -> Option<LossReason> {
        None
    }
    /// The game result once decided: one player left wins, none left is a draw.
    fn game_result(&self, state: &GameState) -> Option<GameResult> {
        let mut alive = state.alive_players();
        match (alive.next(), alive.next()) {
            (None, _) => Some(GameResult::Draw),
            (Some(winner), None) => Some(GameResult::Winner(winner)),
            _ => None,
        }
    }
}

/// The Normal (constructed 60-card) format.
#[derive(Debug, Clone, Copy, Default)]
pub struct Normal;

/// Minimum deck size in Normal.
pub const NORMAL_MIN_DECK: usize = 60;
/// Maximum copies of a non-basic-land card in Normal.
pub const NORMAL_MAX_COPIES: usize = 4;

impl FormatRules for Normal {
    fn name(&self) -> &'static str {
        "normal"
    }

    fn starting_life(&self) -> i32 {
        20
    }

    fn validate_deck(&self, deck: &[Arc<CardDefinition>]) -> Result<(), DeckError> {
        if deck.len() < NORMAL_MIN_DECK {
            return Err(DeckError::TooFewCards {
                min: NORMAL_MIN_DECK,
                got: deck.len(),
            });
        }
        let mut copies: BTreeMap<&str, usize> = BTreeMap::new();
        for card in deck
            .iter()
            .filter(|c| !c.supertypes.contains(&Supertype::Basic))
        {
            *copies.entry(card.name.as_str()).or_default() += 1;
        }
        match copies.into_iter().find(|(_, n)| *n > NORMAL_MAX_COPIES) {
            Some((name, got)) => Err(DeckError::TooManyCopies {
                name: name.to_owned(),
                max: NORMAL_MAX_COPIES,
                got,
            }),
            None => Ok(()),
        }
    }

    fn mulligan_rule(&self) -> MulliganRule {
        MulliganRule {
            hand_size: 7,
            free_mulligans: 0,
        }
    }

    fn skips_first_draw(&self, player_count: usize) -> bool {
        player_count == 2
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the Normal format enforces deck size and copy limits (basics exempt) and reports its constants.
    use super::*;
    use mtg_core::card::CardType;
    use mtg_core::color::Color;
    use rstest::rstest;

    fn deck(spells: &[(&str, usize)], forests: usize) -> Vec<Arc<CardDefinition>> {
        let mut cards = Vec::new();
        for (name, n) in spells {
            let card = Arc::new(CardDefinition::new(*name).with_type(CardType::Creature));
            cards.extend(std::iter::repeat_n(card, *n));
        }
        let forest = Arc::new(CardDefinition::basic_land(Some(Color::Green)));
        cards.extend(std::iter::repeat_n(forest, forests));
        cards
    }

    #[rstest]
    #[case(&[("A", 4), ("B", 4)], 52, true)]
    #[case(&[("A", 4)], 56, true)]
    #[case(&[("A", 4)], 55, false)]
    #[case(&[("A", 5)], 55, false)]
    #[case(&[("A", 1)], 100, true)]
    #[case(&[], 60, true)]
    #[case(&[], 59, false)]
    fn deck_validation_table(
        #[case] spells: &[(&str, usize)],
        #[case] forests: usize,
        #[case] legal: bool,
    ) {
        assert_eq!(Normal.validate_deck(&deck(spells, forests)).is_ok(), legal);
    }

    #[test]
    fn errors_say_what_is_wrong() {
        assert_eq!(
            Normal.validate_deck(&deck(&[("A", 5)], 55)),
            Err(DeckError::TooManyCopies {
                name: "A".into(),
                max: 4,
                got: 5
            })
        );
        assert!(matches!(
            Normal.validate_deck(&[]),
            Err(DeckError::TooFewCards { min: 60, got: 0 })
        ));
    }

    #[rstest]
    #[case(2, true)]
    #[case(3, false)]
    #[case(8, false)]
    fn only_two_player_games_skip_the_first_draw(#[case] players: usize, #[case] skips: bool) {
        assert_eq!(Normal.skips_first_draw(players), skips);
    }

    #[rstest]
    #[case(0, 0, 0)]
    #[case(1, 0, 1)]
    #[case(3, 0, 3)]
    #[case(1, 1, 0)]
    #[case(3, 1, 2)]
    fn london_mulligan_bottoms_one_per_paid_mulligan(
        #[case] mulligans: u32,
        #[case] free: u32,
        #[case] bottom: usize,
    ) {
        let rule = MulliganRule {
            hand_size: 7,
            free_mulligans: free,
        };
        assert_eq!(rule.bottom_count(mulligans), bottom);
    }

    #[test]
    fn normal_constants() {
        assert_eq!(Normal.starting_life(), 20);
        assert_eq!(Normal.mulligan_rule().hand_size, 7);
        assert_eq!(Normal.mulligan_rule().free_mulligans, 0);
        assert_eq!(Normal.extra_zones().len(), 0);
        assert_eq!(Normal.name(), "normal");
    }
}
