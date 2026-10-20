//! Built-in decks for games without a deck builder: mono-colour basics plus vanilla creatures.
//!
//! Every creature is a real card without rules text; names, costs and stats were checked against Scryfall.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::vanilla::{basic_land, creature};
use mtg_core::color::Color;

/// Basic lands in every demo deck.
const LANDS: usize = 24;
/// Copies of each creature in a demo deck.
const COPIES: usize = 4;

/// One creature: name, generic mana, coloured symbols, power, toughness.
type Line = (&'static str, u32, usize, i32, i32);

/// A ready-made 60-card deck for the Normal format.
#[derive(Debug, Clone, Copy)]
pub struct DemoDeck {
    /// Stable identifier used in game requests.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// The deck's only colour.
    pub color: Color,
    creatures: &'static [Line],
}

const GREEN: &[Line] = &[
    ("Runeclaw Bear", 1, 1, 2, 2),
    ("Grizzly Bears", 1, 1, 2, 2),
    ("Elvish Warrior", 0, 2, 2, 3),
    ("Kalonian Tusker", 0, 2, 3, 3),
    ("Centaur Courser", 2, 1, 3, 3),
    ("Alpine Grizzly", 2, 1, 4, 2),
    ("Leatherback Baloth", 0, 3, 4, 5),
    ("Spined Wurm", 4, 1, 5, 4),
    ("Craw Wurm", 4, 2, 6, 4),
];

const RED: &[Line] = &[
    ("Mons's Goblin Raiders", 0, 1, 1, 1),
    ("Goblin Piker", 1, 1, 2, 1),
    ("Goblin Bully", 1, 1, 2, 1),
    ("Gray Ogre", 2, 1, 2, 2),
    ("Viashino Warrior", 3, 1, 4, 2),
    ("Hill Giant", 3, 1, 3, 3),
    ("Cyclops of One-Eyed Pass", 2, 2, 5, 2),
    ("Earth Elemental", 3, 2, 4, 5),
    ("Fire Elemental", 3, 2, 5, 4),
];

const WHITE: &[Line] = &[
    ("Savannah Lions", 0, 1, 2, 1),
    ("Eager Cadet", 0, 1, 1, 1),
    ("Glory Seeker", 1, 1, 2, 2),
    ("Silvercoat Lion", 1, 1, 2, 2),
    ("Pearled Unicorn", 2, 1, 2, 2),
    ("Border Guard", 2, 1, 1, 4),
    ("Foot Soldiers", 3, 1, 2, 4),
    ("Pillarfield Ox", 3, 1, 2, 4),
    ("Siege Mastodon", 4, 1, 3, 5),
];

static DECKS: [DemoDeck; 3] = [
    DemoDeck {
        id: "mono-green",
        name: "Mono Green Stompers",
        color: Color::Green,
        creatures: GREEN,
    },
    DemoDeck {
        id: "mono-red",
        name: "Mono Red Giants",
        color: Color::Red,
        creatures: RED,
    },
    DemoDeck {
        id: "mono-white",
        name: "Mono White Weenies",
        color: Color::White,
        creatures: WHITE,
    },
];

/// Every built-in deck, in a fixed order.
pub fn all() -> &'static [DemoDeck] {
    &DECKS
}

/// The built-in deck with this id.
pub fn find(id: &str) -> Option<&'static DemoDeck> {
    DECKS.iter().find(|deck| deck.id == id)
}

/// The deck a seat made without one holds in its game's recipe until its holder chooses: the seat
/// cannot press Ready before that, so the deck is never played.
pub fn stand_in() -> &'static DemoDeck {
    &DECKS[0]
}

impl DemoDeck {
    /// Number of cards in the deck.
    pub fn size(&self) -> usize {
        LANDS + COPIES * self.creatures.len()
    }

    /// The deck as a list of `(card name, copies)`, lands first.
    pub fn list(&self) -> Vec<(String, usize)> {
        let mut list = vec![(basic_land(self.color).name.clone(), LANDS)];
        list.extend(
            self.creatures
                .iter()
                .map(|line| (line.0.to_owned(), COPIES)),
        );
        list
    }

    /// Builds the 60 cards.
    pub fn cards(&self) -> Vec<Arc<CardDefinition>> {
        let mut cards: Vec<Arc<CardDefinition>> =
            std::iter::repeat_n(basic_land(self.color), LANDS).collect();
        for &(name, generic, pips, power, toughness) in self.creatures {
            let colored = vec![self.color; pips];
            let card = creature(name, generic, &colored, power, toughness);
            cards.extend(std::iter::repeat_n(card, COPIES));
        }
        cards
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every demo deck is a mono-colour, 60-card deck that the Normal format accepts, and `list` describes exactly the cards `cards` builds.
    use std::collections::BTreeMap;

    use super::*;
    use mtg_core::card::{CardType, Supertype};
    use mtg_engine::GameSetup;
    use mtg_engine::format::{FormatRules, Normal};
    use rstest::rstest;

    #[test]
    fn there_are_at_least_two_decks_with_distinct_ids() {
        let ids: std::collections::BTreeSet<_> = all().iter().map(|d| d.id).collect();
        assert!(all().len() >= 2);
        assert_eq!(ids.len(), all().len());
    }

    #[rstest]
    #[case("mono-green")]
    #[case("mono-red")]
    #[case("mono-white")]
    fn decks_are_legal_and_mono_colour(#[case] id: &str) {
        let deck = find(id).unwrap();
        let cards = deck.cards();
        assert_eq!(cards.len(), 60);
        assert_eq!(cards.len(), deck.size());
        assert!(Normal.validate_deck(&cards).is_ok());
        let lands = cards.iter().filter(|c| c.is_type(CardType::Land)).count();
        assert_eq!(lands, 24);
        for card in &cards {
            assert!(
                card.colors().iter().all(|c| c == deck.color),
                "{} strays from {:?}",
                card.name,
                deck.color
            );
            let basic = card.supertypes.contains(&Supertype::Basic);
            assert!(basic || card.is_type(CardType::Creature), "{}", card.name);
            assert!(basic || (card.power.is_some() && card.toughness.is_some()));
            assert!(card.abilities.len() <= 1, "vanilla means no abilities");
        }
    }

    #[test]
    fn list_matches_cards() {
        for deck in all() {
            let mut counted: BTreeMap<String, usize> = BTreeMap::new();
            for card in deck.cards() {
                *counted.entry(card.name.clone()).or_default() += 1;
            }
            let listed: BTreeMap<String, usize> = deck.list().into_iter().collect();
            assert_eq!(counted, listed, "{}", deck.id);
        }
    }

    #[test]
    fn every_pairing_makes_a_valid_setup() {
        for a in all() {
            for b in all() {
                let setup = GameSetup::new(Arc::new(Normal), 1, vec![a.cards(), b.cards()]);
                assert!(setup.validate().is_ok(), "{} vs {}", a.id, b.id);
            }
        }
    }

    #[test]
    fn unknown_ids_are_not_found() {
        for id in ["", "mono-purple", "MONO-GREEN", "mono-green "] {
            assert!(find(id).is_none(), "{id:?}");
        }
    }
}
