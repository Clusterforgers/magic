//! The serializable recipe that rebuilds a game's [`GameSetup`] after a restart.
//!
//! A `GameSetup` holds an `Arc<dyn FormatRules>` and whole card definitions, so it cannot be stored.
//! The recipe names what it was made from instead (format, seed, one deck reference per seat) and is
//! the only way the server builds a setup, for a new game and for a resumed one alike. Its JSON form
//! is stored in `games.recipe`, so field and variant names are an on-disk contract.

use std::sync::Arc;

use mtg_engine::GameSetup;
use mtg_engine::error::SetupError;
use mtg_engine::format::{FormatRules, Normal};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::demo;

/// The format a game follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatKind {
    /// Constructed 60-card games.
    Normal,
}

/// Where one seat's cards come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeckRef {
    /// A built-in demo deck, by its id.
    Demo {
        /// The id in [`demo::all`].
        id: String,
    },
}

/// Everything needed to rebuild a game's [`GameSetup`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameRecipe {
    /// The format's rules.
    pub format: FormatKind,
    /// Seed for shuffles and the first-player roll.
    pub seed: u64,
    /// One deck per seat, in seat order.
    pub decks: Vec<DeckRef>,
}

/// Why a recipe does not describe a playable game.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecipeError {
    /// A seat names a deck the server does not have.
    #[error("seat {seat}: no built-in deck {id:?}")]
    UnknownDeck {
        /// The seat.
        seat: usize,
        /// The id that matched nothing.
        id: String,
    },
    /// The decks and seats do not make a valid game for the format.
    #[error("invalid game setup: {0}")]
    Setup(#[from] SetupError),
}

impl GameRecipe {
    /// A Normal-format recipe with one deck per seat.
    pub fn normal(seed: u64, decks: impl IntoIterator<Item = DeckRef>) -> GameRecipe {
        GameRecipe {
            format: FormatKind::Normal,
            seed,
            decks: decks.into_iter().collect(),
        }
    }

    /// Rebuilds the setup; the same recipe always yields the same game.
    pub fn build(&self) -> Result<GameSetup, RecipeError> {
        let format: Arc<dyn FormatRules> = match self.format {
            FormatKind::Normal => Arc::new(Normal),
        };
        let mut decks = Vec::with_capacity(self.decks.len());
        for (seat, deck) in self.decks.iter().enumerate() {
            decks.push(match deck {
                DeckRef::Demo { id } => demo::find(id)
                    .ok_or_else(|| RecipeError::UnknownDeck {
                        seat,
                        id: id.clone(),
                    })?
                    .cards(),
            });
        }
        let setup = GameSetup::new(format, self.seed, decks);
        setup.validate()?;
        Ok(setup)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a recipe survives its JSON form for every seed and table size, keeps the on-disk shape stable, rebuilds the very game it describes, and refuses what is not playable.
    use mtg_core::card::CardDefinition;
    use mtg_engine::state::GameState;
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    fn demo_decks(ids: &[&str]) -> Vec<DeckRef> {
        ids.iter()
            .map(|id| DeckRef::Demo {
                id: (*id).to_owned(),
            })
            .collect()
    }

    fn names(cards: &[Arc<CardDefinition>]) -> Vec<String> {
        cards.iter().map(|card| card.name.clone()).collect()
    }

    #[rstest]
    #[case::two_seats(0, &["mono-green", "mono-red"])]
    #[case::three_seats(1, &["mono-red", "mono-white", "mono-green"])]
    #[case::mirror(42, &["mono-white", "mono-white"])]
    #[case::twelve_seats(7, &["mono-green"; 12])]
    #[case::big_seed(u64::MAX, &["mono-green", "mono-red"])]
    #[case::seed_past_f64(9_007_199_254_740_993, &["mono-red", "mono-green"])]
    fn recipes_round_trip_through_json_and_rebuild_the_same_game(
        #[case] seed: u64,
        #[case] ids: &[&str],
    ) {
        let recipe = GameRecipe::normal(seed, demo_decks(ids));
        let text = serde_json::to_string(&recipe).unwrap();
        let back: GameRecipe = serde_json::from_str(&text).unwrap();
        assert_eq!(back, recipe);
        let value = serde_json::to_value(&recipe).unwrap();
        assert_eq!(serde_json::from_value::<GameRecipe>(value).unwrap(), recipe);

        let (first, second) = (recipe.build().unwrap(), back.build().unwrap());
        assert_eq!(first.seed, seed);
        assert_eq!(first.players.len(), ids.len());
        let hash = |setup: &GameSetup| GameState::from_setup(setup).unwrap().state_hash();
        assert_eq!(hash(&first), hash(&second));
        for (seat, id) in ids.iter().enumerate() {
            let expected = demo::find(id).unwrap().cards();
            assert_eq!(names(&first.players[seat].library), names(&expected));
        }
    }

    #[test]
    fn the_stored_shape_is_stable() {
        let recipe = GameRecipe::normal(5, demo_decks(&["mono-green", "mono-red"]));
        assert_eq!(
            serde_json::to_value(&recipe).unwrap(),
            json!({
                "format": "normal",
                "seed": 5,
                "decks": [
                    { "kind": "demo", "id": "mono-green" },
                    { "kind": "demo", "id": "mono-red" },
                ],
            })
        );
    }

    #[rstest]
    #[case::no_such_format(json!({"format": "commander", "seed": 1, "decks": []}))]
    #[case::no_such_deck_kind(json!({"format": "normal", "seed": 1, "decks": [{"kind": "list", "cards": []}]}))]
    #[case::negative_seed(json!({"format": "normal", "seed": -1, "decks": []}))]
    #[case::missing_seed(json!({"format": "normal", "decks": []}))]
    #[case::extra_field(json!({"format": "normal", "seed": 1, "decks": [], "banned": []}))]
    #[case::not_an_object(json!("normal"))]
    #[case::null(json!(null))]
    fn recipes_that_do_not_decode_are_refused(#[case] stored: serde_json::Value) {
        assert!(serde_json::from_value::<GameRecipe>(stored).is_err());
    }

    #[rstest]
    #[case::unknown_deck(&["mono-green", "mono-purple"], Some(1))]
    #[case::empty_id(&[""], Some(0))]
    #[case::one_seat(&["mono-green"], None)]
    #[case::no_seats(&[], None)]
    fn recipes_that_are_not_playable_are_refused(
        #[case] ids: &[&str],
        #[case] unknown_seat: Option<usize>,
    ) {
        let error = GameRecipe::normal(1, demo_decks(ids)).build().unwrap_err();
        match unknown_seat {
            Some(seat) => assert!(
                matches!(error, RecipeError::UnknownDeck { seat: s, .. } if s == seat),
                "{error}"
            ),
            None => assert!(matches!(error, RecipeError::Setup(_)), "{error}"),
        }
    }
}
