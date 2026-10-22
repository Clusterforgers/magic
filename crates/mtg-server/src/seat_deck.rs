//! The decks the seats of a lobby play.
//!
//! A [`ChosenDeck`] is a snapshot taken when a deck is chosen: the cards the engine is given, the
//! recipe's name for them (a built-in deck's id, or the oracle id of every card) with their
//! fingerprint, and the three facts the table is told of it. Editing or deleting the stored deck
//! afterwards changes nothing. [`SeatDecks`] keeps one choice per seat while the game is in its
//! lobby; the decks of the seats go into the game's recipe when it starts.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::deck::Issue;
use mtg_core::house::HouseRules;
use mtg_protocol::SeatDeck;

use crate::deck_choice::format_issues;
use crate::demo::{self, DemoDeck};
use crate::recipe::{DeckFingerprint, DeckRef, FingerprintError, FormatKind, GameRecipe};

/// The format key the table is told for a built-in deck.
const DEMO_FORMAT: &str = "normal";

/// A deck chosen for a seat, as it was when it was chosen.
#[derive(Debug, Clone)]
pub struct ChosenDeck {
    cards: Arc<[Arc<CardDefinition>]>,
    commanders: Arc<[Arc<CardDefinition>]>,
    deck: DeckRef,
    fingerprint: DeckFingerprint,
    note: SeatDeck,
}

impl ChosenDeck {
    /// A built-in deck. Every card is one the engine fully supports, so the table has nothing to resolve by hand.
    pub fn demo(deck: &DemoDeck) -> Result<ChosenDeck, FingerprintError> {
        let cards = deck.cards();
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of(&cards)?,
            cards: cards.into(),
            commanders: Arc::from([]),
            deck: DeckRef::Demo {
                id: deck.id.to_owned(),
            },
            note: SeatDeck {
                name: deck.name.to_owned(),
                format: DEMO_FORMAT.to_owned(),
                manual_cards: 0,
            },
        })
    }

    /// What a Commander seat holds until its holder chooses a deck; the seat cannot be ready with it.
    pub fn commander_stand_in() -> Result<ChosenDeck, FingerprintError> {
        let (commander, library) = demo::commander_stand_in();
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of_seat(&library, std::slice::from_ref(&commander))?,
            cards: library.into(),
            commanders: Arc::from([commander]),
            deck: DeckRef::Demo {
                id: demo::COMMANDER_STAND_IN.to_owned(),
            },
            note: SeatDeck {
                name: "No deck".to_owned(),
                format: "commander".to_owned(),
                manual_cards: 0,
            },
        })
    }

    /// A stored deck as assembled: `oracle_ids` names every card of the library in deck order and
    /// `cards` holds their definitions in the same order. `note` is what the table is told.
    pub fn stored(
        oracle_ids: Vec<String>,
        cards: Vec<Arc<CardDefinition>>,
        note: SeatDeck,
    ) -> Result<ChosenDeck, FingerprintError> {
        ChosenDeck::led(oracle_ids, cards, Vec::new(), Vec::new(), note)
    }

    /// Like [`ChosenDeck::stored`], for a deck led by commanders: `commander_ids` names them and
    /// `commanders` holds their definitions in the same order. The seat starts with them in its
    /// command zone, and they are part of the deck's fingerprint.
    pub fn led(
        oracle_ids: Vec<String>,
        cards: Vec<Arc<CardDefinition>>,
        commander_ids: Vec<String>,
        commanders: Vec<Arc<CardDefinition>>,
        note: SeatDeck,
    ) -> Result<ChosenDeck, FingerprintError> {
        Ok(ChosenDeck {
            fingerprint: DeckFingerprint::of_seat(&cards, &commanders)?,
            cards: cards.into(),
            commanders: commanders.into(),
            deck: DeckRef::Oracle {
                cards: oracle_ids,
                commanders: commander_ids,
            },
            note,
        })
    }

    /// What the table is told of the deck.
    pub fn note(&self) -> &SeatDeck {
        &self.note
    }

    /// How the recipe names the deck.
    pub fn reference(&self) -> &DeckRef {
        &self.deck
    }

    /// The fingerprint of the cards the engine is given.
    pub fn fingerprint(&self) -> &DeckFingerprint {
        &self.fingerprint
    }

    /// The library, in deck order.
    pub fn cards(&self) -> &[Arc<CardDefinition>] {
        &self.cards
    }

    /// The commanders the seat starts with in its command zone; none outside Commander.
    pub fn commanders(&self) -> &[Arc<CardDefinition>] {
        &self.commanders
    }

    /// The rules of `format`, under the `house` rules of the game, that the cards break, in the words
    /// of the deck check; none when the format accepts the deck.
    pub fn issues_in(&self, format: FormatKind, house: &HouseRules) -> Vec<Issue> {
        format_issues(format, &self.cards, &self.commanders, house)
    }
}

/// The choice of every seat of a lobby, and the decks the game was made with.
#[derive(Debug)]
pub(crate) struct SeatDecks {
    format: FormatKind,
    house_rules: HouseRules,
    seed: u64,
    /// What each seat plays until its holder chooses, and again once the seat is given back.
    defaults: Vec<Option<ChosenDeck>>,
    current: Vec<Option<ChosenDeck>>,
}

impl SeatDecks {
    /// The lobby of a game made with `defaults`, one entry per seat; a seat without a deck cannot
    /// press Ready.
    pub(crate) fn new(format: FormatKind, seed: u64, defaults: Vec<Option<ChosenDeck>>) -> Self {
        SeatDecks {
            format,
            house_rules: HouseRules::default(),
            seed,
            current: defaults.clone(),
            defaults,
        }
    }

    /// The lobby of a game played under `house_rules`, which every recipe it writes keeps.
    #[must_use]
    pub(crate) fn with_house_rules(mut self, house_rules: HouseRules) -> Self {
        self.house_rules = house_rules;
        self
    }

    /// The lobby of a game stored in `recipe`: only built-in decks come back as chosen, since the
    /// recipe keeps no name for a stored deck; its seat chooses again.
    pub(crate) fn of_recipe(recipe: &GameRecipe) -> Self {
        let defaults = recipe
            .decks
            .iter()
            .map(|deck| match deck {
                DeckRef::Demo { id } => demo::find(id).and_then(|deck| ChosenDeck::demo(deck).ok()),
                DeckRef::Oracle { .. } => None,
            })
            .collect();
        SeatDecks::new(recipe.format, recipe.seed, defaults)
            .with_house_rules(recipe.house_rules.clone())
    }

    /// What the table is told of `seat`'s deck.
    pub(crate) fn note(&self, seat: usize) -> Option<&SeatDeck> {
        self.current.get(seat)?.as_ref().map(ChosenDeck::note)
    }

    /// Whether `seat` has a deck.
    pub(crate) fn has(&self, seat: usize) -> bool {
        self.current.get(seat).is_some_and(Option::is_some)
    }

    /// `seat` plays `deck` from now on; false for a seat the lobby does not have.
    pub(crate) fn choose(&mut self, seat: usize, deck: ChosenDeck) -> bool {
        match self.current.get_mut(seat) {
            Some(slot) => {
                *slot = Some(deck);
                true
            }
            None => false,
        }
    }

    /// `seat` goes back to the deck the game was made with, if it had one; that deck, for the
    /// library to follow.
    pub(crate) fn reset(&mut self, seat: usize) -> Option<&ChosenDeck> {
        let default = self.defaults.get(seat)?.clone();
        *self.current.get_mut(seat)? = default;
        self.current.get(seat)?.as_ref()
    }

    /// Whether any seat plays something else than the deck the game was made with.
    pub(crate) fn changed(&self) -> bool {
        let named = |deck: &Option<ChosenDeck>| {
            deck.as_ref()
                .map(|deck| (deck.deck.clone(), deck.fingerprint))
        };
        self.current
            .iter()
            .map(named)
            .ne(self.defaults.iter().map(named))
    }

    /// The recipe of the game as the seats chose it; none while a seat has no deck.
    pub(crate) fn recipe(&self) -> Option<GameRecipe> {
        let chosen: Option<Vec<&ChosenDeck>> = self.current.iter().map(Option::as_ref).collect();
        let chosen = chosen?;
        Some(GameRecipe {
            format: self.format,
            house_rules: self.house_rules.clone(),
            seed: self.seed,
            decks: chosen.iter().map(|deck| deck.deck.clone()).collect(),
            fingerprints: chosen.iter().map(|deck| deck.fingerprint).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a lobby's decks follow the seats: a seat plays what it chose until it is given back and then what the game was made with, a seat without a deck keeps the recipe from being written, and only a change from the creation decks asks for a new recipe.
    use mtg_cards::vanilla;
    use mtg_core::color::Color;

    use super::*;

    fn demo_deck(id: &str) -> ChosenDeck {
        ChosenDeck::demo(demo::find(id).unwrap()).unwrap()
    }

    fn stored_deck(name: &str) -> ChosenDeck {
        let cards: Vec<Arc<CardDefinition>> =
            std::iter::repeat_n(vanilla::basic_land(Color::Green), 60).collect();
        let note = SeatDeck {
            name: name.to_owned(),
            format: "modern".to_owned(),
            manual_cards: 3,
        };
        ChosenDeck::stored(vec!["forest".to_owned(); 60], cards, note).unwrap()
    }

    fn lobby() -> SeatDecks {
        SeatDecks::new(
            FormatKind::Normal,
            7,
            vec![Some(demo_deck("mono-green")), None],
        )
    }

    #[test]
    fn a_built_in_deck_tells_the_table_its_name_and_no_manual_cards() {
        let deck = demo_deck("mono-red");
        assert_eq!(
            deck.note(),
            &SeatDeck {
                name: "Mono Red Giants".into(),
                format: "normal".into(),
                manual_cards: 0
            }
        );
        assert_eq!(deck.cards().len(), 60);
        assert_eq!(
            deck.fingerprint(),
            &DeckFingerprint::of(deck.cards()).unwrap(),
            "the fingerprint is the one of the cards the engine is given"
        );
    }

    #[test]
    fn a_seat_has_a_deck_once_it_chooses_one() {
        let mut decks = lobby();
        assert!(decks.has(0) && !decks.has(1) && !decks.has(2));
        assert_eq!(decks.note(1), None);
        assert!(decks.choose(1, stored_deck("Burn")));
        assert_eq!(decks.note(1).map(|n| n.name.as_str()), Some("Burn"));
        assert!(!decks.choose(2, stored_deck("Burn")), "no such seat");
    }

    #[test]
    fn a_seat_given_back_returns_to_the_deck_the_game_was_made_with() {
        let mut decks = lobby();
        decks.choose(0, stored_deck("Burn"));
        decks.choose(1, stored_deck("Control"));
        assert_eq!(
            decks.reset(0).map(|deck| deck.note().name.clone()),
            Some("Mono Green Stompers".to_owned())
        );
        assert!(decks.reset(1).is_none(), "it was made without one");
        assert!(!decks.has(1));
    }

    #[test]
    fn the_recipe_waits_for_every_seat_and_names_each_deck_with_its_fingerprint() {
        let mut decks = lobby();
        assert_eq!(decks.recipe(), None, "a seat has no deck");
        decks.choose(1, stored_deck("Burn"));
        let recipe = decks.recipe().unwrap();
        assert_eq!(recipe.seed, 7);
        assert_eq!(
            recipe.decks,
            [
                DeckRef::Demo {
                    id: "mono-green".into()
                },
                DeckRef::oracle(vec!["forest".into(); 60])
            ]
        );
        assert_eq!(recipe.fingerprints.len(), 2);
        assert_eq!(recipe.fingerprints[1], *stored_deck("Burn").fingerprint());
    }

    #[test]
    fn only_a_deck_other_than_the_one_the_game_was_made_with_is_a_change() {
        let mut decks = lobby();
        assert!(!decks.changed());
        decks.choose(0, demo_deck("mono-green"));
        assert!(!decks.changed(), "the same deck again changes nothing");
        decks.choose(0, demo_deck("mono-red"));
        assert!(decks.changed());
        decks.reset(0);
        assert!(!decks.changed());
        decks.choose(1, stored_deck("Burn"));
        assert!(decks.changed(), "a seat made without a deck chose one");
    }

    #[test]
    fn a_lobby_read_back_from_its_recipe_keeps_the_built_in_decks_and_forgets_the_stored_ones() {
        let recipe = GameRecipe::normal(
            3,
            [
                DeckRef::Demo {
                    id: "mono-white".into(),
                },
                DeckRef::oracle(vec!["forest".into(); 60]),
                DeckRef::Demo {
                    id: "no-such-deck".into(),
                },
            ],
        );
        let decks = SeatDecks::of_recipe(&recipe);
        assert_eq!(
            decks.note(0).map(|n| n.name.as_str()),
            Some("Mono White Weenies")
        );
        assert!(!decks.has(1) && !decks.has(2));
        assert!(!decks.changed());
    }
}
