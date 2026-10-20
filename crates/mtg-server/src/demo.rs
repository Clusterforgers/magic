//! Built-in decks for games without a deck builder: basics plus vanilla creatures, and in two decks the corpus cards the engine fully supports.
//!
//! Every vanilla creature is a real card without rules text; names, costs and stats were checked against Scryfall. The corpus cards are assembled from `demo/scryfall.json` and the rules files, as a stored deck's are.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mtg_cards::vanilla::{basic_land, creature};
use mtg_cards::{CardDefinition, PrintedCard, PrintedFace, builtin};
use mtg_core::color::Color;
use mtg_core::mana::ManaType;
use mtg_import::scryfall::{CardRecord, cards};

/// Scryfall objects (trimmed) of the corpus cards the demo decks play.
const SCRYFALL: &str = include_str!("demo/scryfall.json");
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
    /// The deck's main colour, the one of its vanilla creatures.
    pub color: Color,
    /// Basic lands by colour and copies.
    lands: &'static [(Color, usize)],
    creatures: &'static [Line],
    /// Corpus cards by name and copies.
    spells: &'static [(&'static str, usize)],
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

const GREEN_BLACK_CREATURES: &[Line] = &[
    ("Elvish Warrior", 0, 2, 2, 3),
    ("Kalonian Tusker", 0, 2, 3, 3),
    ("Centaur Courser", 2, 1, 3, 3),
    ("Alpine Grizzly", 2, 1, 4, 2),
    ("Leatherback Baloth", 0, 3, 4, 5),
];

const WHITE_BLUE_CREATURES: &[Line] = &[
    ("Silvercoat Lion", 1, 1, 2, 2),
    ("Pearled Unicorn", 2, 1, 2, 2),
    ("Border Guard", 2, 1, 1, 4),
    ("Pillarfield Ox", 3, 1, 2, 4),
    ("Siege Mastodon", 4, 1, 3, 5),
];

static DECKS: [DemoDeck; 5] = [
    DemoDeck {
        id: "mono-green",
        name: "Mono Green Stompers",
        color: Color::Green,
        lands: &[(Color::Green, 24)],
        creatures: GREEN,
        spells: &[],
    },
    DemoDeck {
        id: "mono-red",
        name: "Mono Red Giants",
        color: Color::Red,
        lands: &[(Color::Red, 24)],
        creatures: RED,
        spells: &[],
    },
    DemoDeck {
        id: "mono-white",
        name: "Mono White Weenies",
        color: Color::White,
        lands: &[(Color::White, 24)],
        creatures: WHITE,
        spells: &[],
    },
    DemoDeck {
        id: "green-black",
        name: "Green-Black Midrange",
        color: Color::Green,
        lands: &[(Color::Green, 12), (Color::Black, 12)],
        creatures: GREEN_BLACK_CREATURES,
        spells: &[
            ("Llanowar Elves", 4),
            ("Grizzly Bears", 4),
            ("Doom Blade", 4),
            ("Eternal Witness", 2),
            ("Archon of Cruelty", 2),
        ],
    },
    DemoDeck {
        id: "white-blue",
        name: "White-Blue Control",
        color: Color::White,
        lands: &[(Color::White, 12), (Color::Blue, 12)],
        creatures: WHITE_BLUE_CREATURES,
        spells: &[
            ("Serra Angel", 4),
            ("Swords to Plowshares", 4),
            ("Counterspell", 3),
            ("Brainstorm", 2),
            ("Opt", 2),
            ("Wrath of God", 1),
        ],
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

/// The imported record as the strings assembly reads.
fn printed_card(record: &CardRecord) -> PrintedCard {
    PrintedCard {
        oracle_id: record.oracle_id.to_string(),
        name: record.name.clone(),
        keywords: record.keywords.clone(),
        faces: record
            .faces()
            .map(|face| PrintedFace {
                name: face.name.clone(),
                mana_cost: face.mana_cost.clone(),
                type_line: face.type_line.clone(),
                oracle_text: face.oracle_text.clone(),
                power: face.power.clone(),
                toughness: face.toughness.clone(),
                loyalty: face.loyalty.clone(),
                defense: face.defense.clone(),
                colors: face
                    .colors
                    .iter()
                    .map(|color| ManaType::from(color).symbol().to_string())
                    .collect(),
            })
            .collect(),
    }
}

/// The corpus cards the decks play, by name, as this engine runs them; empty if the embedded data does not load (a test fails then).
fn corpus() -> &'static BTreeMap<String, Arc<CardDefinition>> {
    static CORPUS: OnceLock<BTreeMap<String, Arc<CardDefinition>>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let (Ok(rules), caps) = (builtin::library(), mtg_engine::caps()) else {
            return BTreeMap::new();
        };
        cards(SCRYFALL.as_bytes())
            .filter_map(Result::ok)
            .map(|record| {
                let assembled = rules.assemble(&printed_card(&record), &caps);
                (record.name, Arc::new(assembled.definition))
            })
            .collect()
    })
}

impl DemoDeck {
    /// Number of cards in the deck.
    pub fn size(&self) -> usize {
        let lands: usize = self.lands.iter().map(|&(_, copies)| copies).sum();
        let spells: usize = self.spells.iter().map(|&(_, copies)| copies).sum();
        lands + spells + COPIES * self.creatures.len()
    }

    /// The deck as a list of `(card name, copies)`, lands first.
    pub fn list(&self) -> Vec<(String, usize)> {
        let mut list: Vec<(String, usize)> = self
            .lands
            .iter()
            .map(|&(color, copies)| (basic_land(color).name.clone(), copies))
            .collect();
        list.extend(
            self.creatures
                .iter()
                .map(|line| (line.0.to_owned(), COPIES)),
        );
        list.extend(
            self.spells
                .iter()
                .map(|&(name, copies)| (name.to_owned(), copies)),
        );
        list
    }

    /// Builds the 60 cards.
    pub fn cards(&self) -> Vec<Arc<CardDefinition>> {
        let mut cards: Vec<Arc<CardDefinition>> = Vec::new();
        for &(color, copies) in self.lands {
            cards.extend(std::iter::repeat_n(basic_land(color), copies));
        }
        for &(name, generic, pips, power, toughness) in self.creatures {
            let colored = vec![self.color; pips];
            let card = creature(name, generic, &colored, power, toughness);
            cards.extend(std::iter::repeat_n(card, COPIES));
        }
        for &(name, copies) in self.spells {
            if let Some(card) = corpus().get(name) {
                cards.extend(std::iter::repeat_n(Arc::clone(card), copies));
            }
        }
        cards
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every demo deck is a mono-colour, 60-card deck that the Normal format accepts, and `list` describes exactly the cards `cards` builds.
    use std::collections::BTreeMap;

    use super::*;
    use mtg_cards::support::Support;
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

    #[rstest]
    #[case("green-black", Color::Green, Color::Black)]
    #[case("white-blue", Color::White, Color::Blue)]
    fn two_colour_decks_are_legal_and_use_their_colours(
        #[case] id: &str,
        #[case] first: Color,
        #[case] second: Color,
    ) {
        let deck = find(id).unwrap();
        let cards = deck.cards();
        assert_eq!(cards.len(), 60);
        assert_eq!(cards.len(), deck.size());
        assert!(Normal.validate_deck(&cards).is_ok());
        for card in &cards {
            assert!(
                card.colors().iter().all(|c| c == first || c == second),
                "{} strays from {first:?} and {second:?}",
                card.name
            );
        }
        let spells = cards
            .iter()
            .filter(|c| !c.abilities.is_empty() || !c.rules.is_empty())
            .count();
        assert!(spells >= 16, "{id} plays {spells} cards with abilities");
    }

    #[test]
    fn every_corpus_card_is_one_the_engine_fully_supports() {
        let rules = builtin::library().unwrap();
        let caps = mtg_engine::caps();
        let records: BTreeMap<String, CardRecord> = cards(SCRYFALL.as_bytes())
            .map(|record| record.unwrap())
            .map(|record| (record.name.clone(), record))
            .collect();
        for deck in all() {
            for &(name, _) in deck.spells {
                let record = records
                    .get(name)
                    .unwrap_or_else(|| panic!("{name} missing"));
                let assembled = rules.assemble(&printed_card(record), &caps);
                assert_eq!(assembled.support.support, Support::Verified, "{name}");
            }
        }
    }

    #[test]
    fn the_decks_play_each_other_to_the_end() {
        let green_black = find("green-black").unwrap();
        let white_blue = find("white-blue").unwrap();
        for seed in 0..6 {
            let setup = GameSetup::new(
                Arc::new(Normal),
                seed,
                vec![green_black.cards(), white_blue.cards()],
            );
            let outcome = mtg_testkit::play_game(setup, seed).unwrap_or_else(|e| panic!("{e}"));
            assert!(outcome.turns > 1, "seed {seed}");
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
