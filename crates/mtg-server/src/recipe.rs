//! The serializable recipe that rebuilds a game's [`GameSetup`] after a restart.
//!
//! A `GameSetup` holds an `Arc<dyn FormatRules>` and whole card definitions, so it cannot be stored.
//! The recipe names what it was made from instead (format, seed, one deck reference per seat) and is
//! the only way the server builds a setup, for a new game and for a resumed one alike. Its JSON form
//! is stored in `games.recipe`, so field and variant names are an on-disk contract.
//!
//! A deck reference names a deck by id or by the oracle ids of its cards, so the recipe also keeps a
//! [`DeckFingerprint`] of every seat's cards as they were when the game was created, and building the
//! setup again refuses a deck that no longer matches: a stored event log must never replay under
//! other cards. The fingerprint covers the definitions the engine is given, so an edited card
//! record, a changed rules file and a change in what the engine runs all fail a resume.
//!
//! A deck of oracle ids is built from a [`Catalog`] of definitions that the caller assembled
//! beforehand (`CardSource`), since assembling reads the card database and the recipe does no IO.

mod fingerprint;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::GameSetup;
use mtg_engine::error::SetupError;
use mtg_engine::format::{FormatRules, Normal};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use fingerprint::{DeckFingerprint, FINGERPRINT_VERSION, FingerprintError};

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
    /// A deck of cards named by their Scryfall oracle ids, one entry per card in the deck, in deck
    /// order. The definitions come from the card database and the rules files.
    Oracle {
        /// The oracle id of every card, copies repeated.
        cards: Vec<String>,
    },
}

impl fmt::Display for DeckRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeckRef::Demo { id } => write!(f, "demo deck {id:?}"),
            DeckRef::Oracle { cards } => write!(f, "deck of {} cards by oracle id", cards.len()),
        }
    }
}

/// Assembled card definitions by oracle id: what a deck of oracle ids is built from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    cards: BTreeMap<String, Arc<CardDefinition>>,
}

impl Catalog {
    /// A catalog of these cards.
    pub fn new(cards: impl IntoIterator<Item = (String, Arc<CardDefinition>)>) -> Catalog {
        Catalog {
            cards: cards.into_iter().collect(),
        }
    }

    /// The card with this oracle id.
    pub fn get(&self, id: &str) -> Option<&Arc<CardDefinition>> {
        self.cards.get(id)
    }

    /// How many cards the catalog holds.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether the catalog holds no card.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// The cards of a deck in deck order, copies sharing one definition; when some are missing, the
    /// ids of all of them, once each.
    fn deck(&self, ids: &[String]) -> Result<Vec<Arc<CardDefinition>>, Vec<String>> {
        let mut missing: Vec<String> = Vec::new();
        let mut cards = Vec::with_capacity(ids.len());
        for id in ids {
            match self.cards.get(id) {
                Some(card) => cards.push(Arc::clone(card)),
                None if !missing.contains(id) => missing.push(id.clone()),
                None => {}
            }
        }
        if missing.is_empty() {
            Ok(cards)
        } else {
            Err(missing)
        }
    }
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
    /// What each seat's deck held when the game was created, in seat order. Empty for a recipe made
    /// without them or stored before they existed; such a recipe builds without a check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fingerprints: Vec<DeckFingerprint>,
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
    /// A seat's deck of oracle ids names cards the catalog does not hold.
    #[error("seat {seat}: no card for the oracle id(s) {}", ids.join(", "))]
    UnknownCards {
        /// The seat.
        seat: usize,
        /// Every oracle id the catalog lacks, once each, in deck order.
        ids: Vec<String>,
    },
    /// A seat's deck no longer holds the cards the game was created with.
    #[error(
        "the deck changed: seat {seat} ({deck}) was {stored} when the game was created and is {current} now"
    )]
    DeckChanged {
        /// The first seat whose deck differs.
        seat: usize,
        /// The deck that seat plays, as the recipe names it.
        deck: String,
        /// The fingerprint stored with the recipe.
        stored: DeckFingerprint,
        /// The fingerprint of the deck as it is now.
        current: DeckFingerprint,
    },
    /// The recipe keeps fingerprints but not one per deck.
    #[error("the recipe has {decks} decks but {fingerprints} deck fingerprints")]
    FingerprintCount {
        /// Decks in the recipe.
        decks: usize,
        /// Fingerprints in the recipe.
        fingerprints: usize,
    },
    /// A deck could not be fingerprinted.
    #[error(transparent)]
    Fingerprint(#[from] FingerprintError),
    /// The decks and seats do not make a valid game for the format.
    #[error("invalid game setup: {0}")]
    Setup(#[from] SetupError),
}

impl GameRecipe {
    /// A Normal-format recipe with one deck per seat and no fingerprints; a game that is being
    /// created takes them with [`GameRecipe::with_fingerprints`].
    pub fn normal(seed: u64, decks: impl IntoIterator<Item = DeckRef>) -> GameRecipe {
        GameRecipe {
            format: FormatKind::Normal,
            seed,
            decks: decks.into_iter().collect(),
            fingerprints: Vec::new(),
        }
    }

    /// The recipe with the fingerprint of every seat's deck as the server has it now, replacing any
    /// it carried. A game takes them once, when it is created. A deck of oracle ids needs
    /// [`GameRecipe::with_fingerprints_from`].
    pub fn with_fingerprints(self) -> Result<GameRecipe, RecipeError> {
        self.with_fingerprints_from(&Catalog::default())
    }

    /// Like [`GameRecipe::with_fingerprints`], with the definitions of the oracle ids the decks name.
    pub fn with_fingerprints_from(mut self, catalog: &Catalog) -> Result<GameRecipe, RecipeError> {
        let decks = self.resolve(catalog)?;
        self.fingerprints = decks
            .iter()
            .map(|cards| DeckFingerprint::of(cards))
            .collect::<Result<_, _>>()?;
        Ok(self)
    }

    /// The oracle ids that the decks of the recipe name, once each, in the order they first appear;
    /// empty when no seat plays a deck of oracle ids, so no card database is needed.
    pub fn oracle_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.decks
            .iter()
            .filter_map(|deck| match deck {
                DeckRef::Oracle { cards } => Some(cards),
                DeckRef::Demo { .. } => None,
            })
            .flatten()
            .filter(|id| seen.insert(id.as_str()))
            .cloned()
            .collect()
    }

    /// Whether the recipe carries fingerprints, so that [`GameRecipe::build`] checks its decks.
    pub fn is_fingerprinted(&self) -> bool {
        !self.fingerprints.is_empty()
    }

    /// Rebuilds the setup; the same recipe always yields the same game.
    ///
    /// A fingerprinted recipe builds only while every deck still hashes to its fingerprint, since
    /// the events stored for the game were made under the cards it was created with.
    pub fn build(&self) -> Result<GameSetup, RecipeError> {
        self.build_with(&Catalog::default())
    }

    /// Like [`GameRecipe::build`], with the definitions of the oracle ids the decks name; resume
    /// builds a game with decks of oracle ids this way, from cards assembled out of the database as
    /// it is now.
    pub fn build_with(&self, catalog: &Catalog) -> Result<GameSetup, RecipeError> {
        let format: Arc<dyn FormatRules> = match self.format {
            FormatKind::Normal => Arc::new(Normal),
        };
        let decks = self.resolve(catalog)?;
        self.check(&decks)?;
        let setup = GameSetup::new(format, self.seed, decks);
        setup.validate()?;
        Ok(setup)
    }

    /// The cards of every seat's deck, in deck order.
    fn resolve(&self, catalog: &Catalog) -> Result<Vec<Vec<Arc<CardDefinition>>>, RecipeError> {
        let mut decks = Vec::with_capacity(self.decks.len());
        for (seat, deck) in self.decks.iter().enumerate() {
            decks.push(match deck {
                DeckRef::Demo { id } => demo::find(id)
                    .ok_or_else(|| RecipeError::UnknownDeck {
                        seat,
                        id: id.clone(),
                    })?
                    .cards(),
                DeckRef::Oracle { cards } => catalog
                    .deck(cards)
                    .map_err(|ids| RecipeError::UnknownCards { seat, ids })?,
            });
        }
        Ok(decks)
    }

    /// Compares each deck with its stored fingerprint; a recipe without fingerprints passes.
    fn check(&self, decks: &[Vec<Arc<CardDefinition>>]) -> Result<(), RecipeError> {
        if !self.is_fingerprinted() {
            return Ok(());
        }
        if self.fingerprints.len() != decks.len() {
            return Err(RecipeError::FingerprintCount {
                decks: decks.len(),
                fingerprints: self.fingerprints.len(),
            });
        }
        let seats = self.decks.iter().zip(decks).zip(&self.fingerprints);
        for (seat, ((deck, cards), stored)) in seats.enumerate() {
            let current = DeckFingerprint::of(cards)?;
            if current != *stored {
                return Err(RecipeError::DeckChanged {
                    seat,
                    deck: deck.to_string(),
                    stored: *stored,
                    current,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a recipe survives its JSON form for every seed and table size, keeps the on-disk shape stable, rebuilds the very game it describes, refuses what is not playable, and refuses a deck that no longer matches the fingerprint it was created with while a recipe stored before fingerprints still builds; a deck of oracle ids builds from a catalog, names every card the catalog lacks, and fails the game when a definition changes.
    use mtg_core::color::Color;
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

    /// The fingerprint a deck had before a deploy gave one of its cards a different power.
    fn before_a_power_changed(id: &str) -> DeckFingerprint {
        let mut cards = demo::find(id).unwrap().cards();
        let mut card = (*cards[30]).clone();
        card.power = card.power.map(|power| power + 1);
        cards[30] = Arc::new(card);
        DeckFingerprint::of(&cards).unwrap()
    }

    fn fingerprinted(seed: u64, ids: &[&str]) -> GameRecipe {
        GameRecipe::normal(seed, demo_decks(ids))
            .with_fingerprints()
            .unwrap()
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
    #[case::two_seats(0, &["mono-green", "mono-red"])]
    #[case::three_seats(1, &["mono-red", "mono-white", "mono-green"])]
    #[case::mirror(42, &["mono-white", "mono-white"])]
    #[case::twelve_seats(7, &["mono-green"; 12])]
    #[case::big_seed(u64::MAX, &["mono-green", "mono-red"])]
    fn fingerprinted_recipes_round_trip_and_rebuild_the_same_game(
        #[case] seed: u64,
        #[case] ids: &[&str],
    ) {
        let recipe = fingerprinted(seed, ids);
        assert!(recipe.is_fingerprinted());
        assert_eq!(recipe.fingerprints.len(), ids.len());
        let back: GameRecipe =
            serde_json::from_str(&serde_json::to_string(&recipe).unwrap()).unwrap();
        assert_eq!(back, recipe);

        let hash = |recipe: &GameRecipe| {
            let setup = recipe.build().unwrap();
            GameState::from_setup(&setup).unwrap().state_hash()
        };
        let plain = GameRecipe::normal(seed, demo_decks(ids));
        assert_eq!(hash(&back), hash(&plain), "fingerprints change no game");
    }

    #[test]
    fn a_fingerprinted_recipe_stores_one_fingerprint_per_seat_in_seat_order() {
        let recipe = fingerprinted(5, &["mono-green", "mono-red"]);
        assert_eq!(
            serde_json::to_value(&recipe).unwrap(),
            json!({
                "format": "normal",
                "seed": 5,
                "decks": [
                    { "kind": "demo", "id": "mono-green" },
                    { "kind": "demo", "id": "mono-red" },
                ],
                "fingerprints": [
                    "v1:7cab379fe30b1810069843fcf113cbbbfa8f11784d9772bdd68b49c77e0c1ceb",
                    "v1:9f1f1eb342c24e939e463142ee4dadfd1c63ff8a9deaceda80f5ca501b26088f",
                ],
            })
        );
    }

    #[test]
    fn a_recipe_stored_before_fingerprints_existed_decodes_and_builds_unchecked() {
        let stored = json!({
            "format": "normal",
            "seed": 9,
            "decks": [
                { "kind": "demo", "id": "mono-white" },
                { "kind": "demo", "id": "mono-red" },
            ],
        });
        let recipe: GameRecipe = serde_json::from_value(stored.clone()).unwrap();
        assert!(!recipe.is_fingerprinted());
        assert!(recipe.build().is_ok());
        assert_eq!(serde_json::to_value(&recipe).unwrap(), stored);
    }

    #[test]
    fn taking_fingerprints_again_replaces_stale_ones() {
        let mut recipe = fingerprinted(3, &["mono-green", "mono-red"]);
        let fresh = recipe.clone();
        recipe.fingerprints = vec![before_a_power_changed("mono-green"); 2];
        assert!(recipe.build().is_err());
        assert_eq!(recipe.with_fingerprints().unwrap(), fresh);
    }

    #[rstest]
    #[case::first_seat(&[0], 0)]
    #[case::second_seat(&[1], 1)]
    #[case::third_seat(&[2], 2)]
    #[case::the_first_of_two(&[1, 2], 1)]
    #[case::all_of_them(&[0, 1, 2], 0)]
    fn a_deck_that_no_longer_matches_is_refused_naming_its_first_seat(
        #[case] stale: &[usize],
        #[case] reported: usize,
    ) {
        let ids = ["mono-green", "mono-red", "mono-white"];
        let mut recipe = fingerprinted(1, &ids);
        let current = recipe.fingerprints[reported];
        for &seat in stale {
            recipe.fingerprints[seat] = before_a_power_changed(ids[seat]);
        }
        let stored = recipe.fingerprints[reported];
        let deck = format!("demo deck {:?}", ids[reported]);
        let error = recipe.build().unwrap_err();
        assert_eq!(
            error,
            RecipeError::DeckChanged {
                seat: reported,
                deck: deck.clone(),
                stored,
                current
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "the deck changed: seat {reported} ({deck}) was {stored} when the game was created and is {current} now"
            )
        );
    }

    #[rstest]
    #[case::too_few(1)]
    #[case::too_many(3)]
    fn fingerprints_that_are_not_one_per_deck_are_refused(#[case] count: usize) {
        let mut recipe = fingerprinted(1, &["mono-green", "mono-red"]);
        recipe.fingerprints.resize(count, recipe.fingerprints[0]);
        assert_eq!(
            recipe.build().unwrap_err(),
            RecipeError::FingerprintCount {
                decks: 2,
                fingerprints: count
            }
        );
    }

    #[test]
    fn an_unknown_deck_is_reported_before_any_fingerprint_is_compared() {
        let mut recipe = fingerprinted(1, &["mono-green", "mono-red"]);
        recipe.decks[1] = DeckRef::Demo {
            id: "mono-purple".into(),
        };
        assert!(matches!(
            recipe.build().unwrap_err(),
            RecipeError::UnknownDeck { seat: 1, .. }
        ));
        assert!(matches!(
            recipe.with_fingerprints().unwrap_err(),
            RecipeError::UnknownDeck { seat: 1, .. }
        ));
    }

    /// Sixty cards named by id: forty `forest` lands, and twenty creatures of one id each.
    fn ids() -> Vec<String> {
        (0..60)
            .map(|i| {
                if i < 40 {
                    "forest".into()
                } else {
                    format!("bear-{i}")
                }
            })
            .collect()
    }

    fn oracle(ids: Vec<String>) -> DeckRef {
        DeckRef::Oracle { cards: ids }
    }

    /// The definitions of `ids()`; a creature's power says which of two builds this is.
    fn catalog(power: i32) -> Catalog {
        let mut cards = vec![(
            "forest".to_owned(),
            mtg_cards::vanilla::basic_land(Color::Green),
        )];
        for i in 40..60 {
            let bear =
                mtg_cards::vanilla::creature(&format!("Bear {i}"), 1, &[Color::Green], power, 2);
            cards.push((format!("bear-{i}"), bear));
        }
        Catalog::new(cards)
    }

    fn oracle_recipe(power: i32) -> GameRecipe {
        GameRecipe::normal(8, [oracle(ids()), oracle(ids())])
            .with_fingerprints_from(&catalog(power))
            .unwrap()
    }

    #[test]
    fn the_stored_shape_of_a_deck_of_oracle_ids_is_stable() {
        let recipe = GameRecipe::normal(5, [oracle(vec!["a".into(), "b".into(), "a".into()])]);
        assert_eq!(
            serde_json::to_value(&recipe).unwrap(),
            json!({
                "format": "normal",
                "seed": 5,
                "decks": [{ "kind": "oracle", "cards": ["a", "b", "a"] }],
            })
        );
        let back: GameRecipe =
            serde_json::from_value(serde_json::to_value(&recipe).unwrap()).unwrap();
        assert_eq!(back, recipe);
    }

    #[test]
    fn the_oracle_ids_of_a_recipe_come_once_each_in_order_and_only_from_oracle_decks() {
        let recipe = GameRecipe::normal(
            1,
            [
                DeckRef::Demo {
                    id: "mono-red".into(),
                },
                oracle(vec!["b".into(), "a".into(), "b".into()]),
                oracle(vec!["c".into(), "a".into()]),
            ],
        );
        assert_eq!(recipe.oracle_ids(), ["b", "a", "c"]);
        assert!(
            fingerprinted(1, &["mono-green"]).oracle_ids().is_empty(),
            "demo decks need no cards"
        );
    }

    #[test]
    fn a_deck_of_oracle_ids_builds_from_a_catalog_and_copies_share_a_definition() {
        let setup = oracle_recipe(2).build_with(&catalog(2)).unwrap();
        assert_eq!(setup.players.len(), 2);
        let library = &setup.players[0].library;
        assert_eq!(library.len(), 60);
        assert!(
            Arc::ptr_eq(&library[0], &library[39]),
            "forty copies of one card"
        );
        assert_eq!(names(library)[40], "Bear 40");
        // The same recipe and catalog make the same game.
        let hash = |setup: &GameSetup| GameState::from_setup(setup).unwrap().state_hash();
        assert_eq!(
            hash(&setup),
            hash(&oracle_recipe(2).build_with(&catalog(2)).unwrap())
        );
    }

    #[test]
    fn without_its_catalog_a_deck_of_oracle_ids_does_not_build() {
        let recipe = oracle_recipe(2);
        assert!(matches!(
            recipe.build().unwrap_err(),
            RecipeError::UnknownCards { seat: 0, .. }
        ));
        assert!(matches!(
            GameRecipe::normal(1, [oracle(ids())])
                .with_fingerprints()
                .unwrap_err(),
            RecipeError::UnknownCards { seat: 0, .. }
        ));
    }

    #[test]
    fn every_missing_card_is_named_once_for_the_first_seat_that_lacks_any() {
        let mut short = catalog(2);
        short.cards.remove("bear-41");
        short.cards.remove("bear-57");
        let recipe = GameRecipe::normal(1, [oracle(vec!["forest".into(); 60]), oracle(ids())]);
        let error = recipe.build_with(&short).unwrap_err();
        assert_eq!(
            error,
            RecipeError::UnknownCards {
                seat: 1,
                ids: vec!["bear-41".into(), "bear-57".into()]
            }
        );
        assert_eq!(
            error.to_string(),
            "seat 1: no card for the oracle id(s) bear-41, bear-57"
        );
        let twice = vec!["ghost".to_owned(); 5];
        let error = GameRecipe::normal(1, [oracle(twice)])
            .build_with(&Catalog::default())
            .unwrap_err();
        assert_eq!(
            error,
            RecipeError::UnknownCards {
                seat: 0,
                ids: vec!["ghost".into()]
            }
        );
    }

    #[test]
    fn a_changed_definition_fails_the_deck_like_a_changed_demo_deck() {
        let recipe = oracle_recipe(2);
        assert!(recipe.is_fingerprinted());
        assert!(recipe.build_with(&catalog(2)).is_ok());
        let error = recipe.build_with(&catalog(3)).unwrap_err();
        match error {
            RecipeError::DeckChanged {
                seat,
                deck,
                stored,
                current,
            } => {
                assert_eq!((seat, deck.as_str()), (0, "deck of 60 cards by oracle id"));
                assert_ne!(stored, current);
                assert_eq!(current, oracle_recipe(3).fingerprints[0]);
            }
            other => panic!("{other}"),
        }
    }

    #[test]
    fn oracle_decks_need_no_fingerprints_to_build_and_do_not_disturb_demo_games() {
        let mut recipe = oracle_recipe(2);
        recipe.fingerprints.clear();
        assert!(
            recipe.build_with(&catalog(9)).is_ok(),
            "unchecked like any recipe without them"
        );
        let plain = fingerprinted(2, &["mono-green", "mono-red"]);
        let setup = plain.build_with(&catalog(2)).unwrap();
        assert_eq!(
            setup.players.len(),
            2,
            "a catalog nobody asked for is ignored"
        );
    }

    #[rstest]
    #[case::no_such_format(json!({"format": "commander", "seed": 1, "decks": []}))]
    #[case::no_such_deck_kind(json!({"format": "normal", "seed": 1, "decks": [{"kind": "list", "cards": []}]}))]
    #[case::negative_seed(json!({"format": "normal", "seed": -1, "decks": []}))]
    #[case::missing_seed(json!({"format": "normal", "decks": []}))]
    #[case::extra_field(json!({"format": "normal", "seed": 1, "decks": [], "banned": []}))]
    #[case::fingerprint_of_another_version(json!({"format": "normal", "seed": 1, "decks": [], "fingerprints": ["v2:7cab379fe30b1810069843fcf113cbbbfa8f11784d9772bdd68b49c77e0c1ceb"]}))]
    #[case::fingerprint_that_is_not_hex(json!({"format": "normal", "seed": 1, "decks": [], "fingerprints": ["v1:green"]}))]
    #[case::fingerprint_that_is_not_text(json!({"format": "normal", "seed": 1, "decks": [], "fingerprints": [7]}))]
    #[case::fingerprints_that_are_not_a_list(json!({"format": "normal", "seed": 1, "decks": [], "fingerprints": "v1:00"}))]
    #[case::null_fingerprints(json!({"format": "normal", "seed": 1, "decks": [], "fingerprints": null}))]
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
